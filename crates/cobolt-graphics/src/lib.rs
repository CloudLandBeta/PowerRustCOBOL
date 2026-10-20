// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! `cobolt-graphics`: Canvas 2D, Canvas 3D and the graphics objects (WASM-018).
//!
//! Two controls - **Canvas2D** and **Canvas3D** - and five non-visual objects -
//! **ImageBuffer**, **Plot**, **Scene3D**, **Physics2D** and **Physics3D** - for a
//! RustCOBOL program, implemented **once**, in pure Rust, with no dependency on egui,
//! on `web-sys` or on the file system. The desktop host (the egui form host), the
//! interpreter, the generated module and the browser player all use this crate; what
//! they add is a surface to show the picture on, input events, a frame clock and
//! the bytes of the assets.
//!
//! # How a host uses it
//!
//! A host makes one [`Registry`], gives it its services, and then turns every call a
//! COBOL program makes on a graphics object into [`Registry::invoke`]:
//!
//! ```
//! use cobolt_graphics::{Limits, ObjectKind, Registry, Value};
//!
//! let mut reg = Registry::new(Limits::default());
//! // Form number 1 makes a 200 x 100 canvas; the handle is an ordinary number.
//! let canvas = reg.create(ObjectKind::Canvas2D, 1, &[Value::Int(200), Value::Int(100)]).unwrap();
//!
//! // CANVAS::"SetFillColor"("#2060C0")   CANVAS::"FillRect"(10 10 80 40)   CANVAS::"Present"()
//! reg.invoke(canvas, "SetFillColor", &[Value::Text("#2060C0".into())]).unwrap();
//! reg.invoke(canvas, "FillRect", &[10.into(), 10.into(), 80.into(), 40.into()]).unwrap();
//! reg.invoke(canvas, "Present", &[]).unwrap();
//!
//! // The picture is there: GetPixel answers 0xRRGGBBAA.
//! let pixel = reg.invoke(canvas, "GetPixel", &[20.into(), 20.into()]).unwrap();
//! assert_eq!(pixel, Value::Int(0x2060C0FF));
//!
//! // When the form is destroyed, everything it made goes with it.
//! assert_eq!(reg.release_owner(1), 1);
//! assert!(reg.is_empty());
//! ```
//!
//! Hosts that want more than numbers use the typed API under the same names: [`Canvas2D`]
//! (and its [`display_list`]), [`imagebuf::ImageBuffer`], [`plot::Plot`], [`scene3d::Scene3D`]
//! and [`scene3d::Canvas3D`], [`physics::Physics2D`] and [`physics::Physics3D`].
//!
//! # The pieces
//!
//! | Area | Module | What it is |
//! |---|---|---|
//! | Display list | [`display_list`] | The versioned binary encoding every drawing method records into (`PRGD`, version 1), defined once for the encoder, the decoder and the executor |
//! | Canvas 2D | [`canvas`], [`canvas2d`] | The rasteriser: `tiny-skia`, CPU, anti-aliased, deterministic, in parallel bands for large canvases |
//! | Text | [`text`] | Shaping (`harfrust`), outlines (`skrifa`), bidirectional text (`unicode-bidi`), line breaking (`icu_segmenter`), fonts from the host (`fontdb`); `MeasureText` is exactly what `DrawText` occupies |
//! | Images | [`imagebuf`] | PNG, JPEG, GIF, WebP, BMP and QOI through pure-Rust codecs, resampling, filters, blur, compositing; a pixel limit checked from the file header |
//! | Charts | [`plot`] | `plotters` for layout, our own backends so the canvas rasteriser and a text engine we control draw everything; twelve chart kinds; SVG; a COBOL table passed as one block |
//! | 3D scene | [`scene3d`] | Nodes, primitives, glTF/GLB with skins and animation, materials, lights, cameras, culling, shadow fitting, picking through `parry3d` |
//! | 3D renderer | [`scene3d`] (`Canvas3D`) | `wgpu`: PBR shading, shadow maps, fog, instancing, GPU skinning, MSAA, offscreen read-back; reports its [`scene3d::Backend`] |
//! | Physics | [`physics`] | `rapier2d` and `rapier3d` with `enhanced-determinism` and Rayon: worlds, bodies, shapes, joints, ray casts, a fixed step, events |
//! | Handles and limits | [`handles`], [`limits`], [`registry`] | Whole-number handles with generations, one [`Limits`] for every size, owners that release what a form made |
//! | The COBOL boundary | [`value`], [`registry`] | [`Value`], [`Registry::invoke`] and the method tables below |
//!
//! # Features
//!
//! | Feature | Default | Enables |
//! |---|---|---|
//! | `parallel` | yes | Rayon for per-pixel and per-row work and for the physics solver; **identical results** without it, and on `wasm32` without the `atomics` target feature the crate runs sequentially whatever this says |
//! | `text` | yes | `text`; needed by `plot` |
//! | `imagebuf` | yes | `imagebuf` |
//! | `plot` | yes | `plot` (needs `text`) |
//! | `physics` | yes | `physics` |
//! | `scene3d-core` | via `scene3d` | The scene graph, glTF loader, culling, shadow fitting and picking, with **no GPU code**: plain data and maths that run in every test |
//! | `scene3d` | yes | `scene3d-core` plus the `wgpu` renderer and `Canvas3D` |
//!
//! The crate builds natively and for `wasm32-unknown-unknown`, in every combination of those features.
//!
//! # Determinism (WASM-018 R20)
//!
//! Work split across threads - image filters and resampling, plot rendering, canvas bands,
//! the physics solver - gives **the same bytes for one thread and for many**: per-pixel work is
//! independent, reductions run in a fixed order, the physics world is built with
//! `enhanced-determinism`, and the shadow and blur kernels use no platform maths. The CPU
//! rasteriser's pictures are compared exactly with golden images; the GPU's are compared
//! within a stated tolerance (see `tests/scene3d_gpu.rs`).
//!
//! # Safety and limits (WASM-018 R15)
//!
//! Every size - canvas, decoded image, texture, model, nodes, triangles, bodies, display list,
//! plot points, total memory - is checked against [`Limits`] *before* anything is allocated, and
//! the error names the setting that raises it. The crate reads no file and opens no connection:
//! assets come through [`resources::AssetHost`]. Images and models are decoded by pure-Rust code
//! that the test suite feeds thousands of damaged inputs. A graphics device that is missing or
//! lost is a message and an event, never a panic.
//!
//! # The COBOL-facing methods
//!
//! The tables below are the method names a program may call, generated from the very rows that
//! dispatch the calls (`Registry::all_methods`); a test fails if they drift apart
//! (`DOC_UPDATE=1 cargo test -p cobolt-graphics --test api` rewrites them). Kinds: `int` and
//! `num` are numbers, `handle` a whole number from 1 that names something this object or
//! another made, `color` a number `0xRRGGBBAA` or text (`#336699`, `#33669980`, `red`),
//! `flag` 0 or 1, `numbers` and `ints` lists, `bytes` a block. A parameter in `[brackets]` may
//! be left out. Method names match without regard to case.
//!
//! Canvas2D also has the drawing commands of the display list (`FillRect`, `LineTo`, ...) under
//! the names the display-list table gives them; they are recorded, and drawn at `Present`,
//! `Flush` or a read-back, in one call.
//!
//! <!-- methods:begin -->
//! ### Canvas2D (75 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `Resize` | width: int, height: int, [pixel_ratio: num] | - | New size in logical pixels (and device pixel ratio); clears the picture and the recorded list. |
//! | `SetPixelRatio` | ratio: num | - | Change the device pixel ratio (clears the picture, like Resize). |
//! | `GetWidth` | - | int | The width in logical pixels. |
//! | `GetHeight` | - | int | The height in logical pixels. |
//! | `GetPixelRatio` | - | num | Device pixels per logical pixel. |
//! | `Flush` | - | int | Draw everything recorded so far; the answer is how many commands ran. |
//! | `Discard` | - | - | Forget what was recorded without drawing it. |
//! | `PendingCommands` | - | int | How many commands are recorded and not yet drawn. |
//! | `HostCalls` | - | int | How many times the rasteriser has been called (a flush is one call, however long the list). |
//! | `TakePresented` | - | int | 1 if Present ran since the last call, else 0 (the host repaints when it is 1). |
//! | `ToImage` | - | handle | Draw what is recorded and return the whole picture as a new ImageBuffer. |
//! | `Capture` | x: num, y: num, width: num, height: num | handle | A rectangle of the picture as a new ImageBuffer (0 when the rectangle is empty). |
//! | `GetPixel` | x: num, y: num | color | The colour at a point as 0xRRGGBBAA, or -1 outside the canvas. |
//! | `IsPointInPath` | x: num, y: num, [even_odd: flag] | int | 1 if the point is inside the current path (0 non-zero rule, 1 even-odd). |
//! | `IsPointInStroke` | x: num, y: num | int | 1 if the point is on the stroke of the current path. |
//! | `MeasureText` | text: text | numbers | Metrics of a text in the current font: width, ink left, ink right, ink ascent, ink descent, font ascent, font descent, lines. |
//! | `LoadFont` | bytes: bytes | int | Register the faces of a font file (TTF, OTF, TTC); the answer is how many were added. |
//! | `LoadFontAsset` | name: text | int | Register the font in an asset by name. |
//! | `GetDiagnostics` | - | text | What was skipped since the last call (a missing image, no fonts ...), one per line. |
//! | `Save` | - | - | Push the graphics state (transform, styles, clip, text settings). |
//! | `Restore` | - | - | Pop the graphics state. |
//! | `Reset` | - | - | Back to the default state: empties the stack, the clip and the path (pixels stay). |
//! | `Clear` | color: color | - | Fill the whole canvas with a colour, ignoring transform, clip and blend mode. |
//! | `Present` | - | - | A frame boundary: the host may show the picture now. |
//! | `SetSmoothing` | quality: int | - | Image resampling quality: 0 off, 1 low, 2 high. |
//! | `BeginPath` | - | - | Start a new empty path. |
//! | `MoveTo` | x: num, y: num | - | Start a sub-path. |
//! | `LineTo` | x: num, y: num | - | A line to the point. |
//! | `QuadTo` | cx: num, cy: num, x: num, y: num | - | A quadratic curve. |
//! | `CubicTo` | c1x: num, c1y: num, c2x: num, c2y: num, x: num, y: num | - | A cubic curve. |
//! | `ArcTo` | x1: num, y1: num, x2: num, y2: num, radius: num | - | A rounded corner between two lines (HTML `arcTo`). |
//! | `Arc` | x: num, y: num, radius: num, start: num, end: num, ccw: int | - | A circular arc; angles in radians, `ccw` 1 for counter-clockwise. |
//! | `Ellipse` | x: num, y: num, rx: num, ry: num, rotation: num, start: num, end: num, ccw: int | - | An elliptical arc, rotated by `rotation` radians. |
//! | `Rect` | x: num, y: num, w: num, h: num | - | A closed rectangle sub-path. |
//! | `RoundRect` | x: num, y: num, w: num, h: num, tl: num, tr: num, br: num, bl: num | - | A closed rounded rectangle; the four radii are top-left, top-right, bottom-right, bottom-left. |
//! | `ClosePath` | - | - | Close the current sub-path. |
//! | `Fill` | rule: int | - | Fill the current path with the fill style; `rule` 0 non-zero, 1 even-odd. |
//! | `Stroke` | - | - | Stroke the current path with the stroke style. |
//! | `Clip` | rule: int | - | Intersect the clip with the current path. |
//! | `FillRect` | x: num, y: num, w: num, h: num | - | Fill a rectangle (does not touch the current path). |
//! | `StrokeRect` | x: num, y: num, w: num, h: num | - | Stroke a rectangle. |
//! | `ClearRect` | x: num, y: num, w: num, h: num | - | Make a rectangle transparent. |
//! | `DrawLine` | x1: num, y1: num, x2: num, y2: num | - | Stroke a line. |
//! | `DrawCircle` | x: num, y: num, radius: num, mode: int | - | A circle; `mode` bit 0 fills, bit 1 strokes. |
//! | `DrawPolygon` | points: numbers, mode: int | - | A polygon or polyline; `mode` bit 0 fills, bit 1 strokes, bit 2 closes. |
//! | `SetFillColor` | color: color | - | Fill with a solid colour. |
//! | `SetStrokeColor` | color: color | - | Stroke with a solid colour. |
//! | `SetLineWidth` | width: num | - | Stroke width. |
//! | `SetLineCap` | cap: int | - | 0 butt, 1 round, 2 square. |
//! | `SetLineJoin` | join: int | - | 0 miter, 1 round, 2 bevel. |
//! | `SetMiterLimit` | limit: num | - | Miter limit. |
//! | `SetDash` | intervals: numbers, offset: num | - | Dash pattern (empty for solid) and its offset. |
//! | `SetAlpha` | alpha: num | - | Global alpha, 0 to 1. |
//! | `SetBlendMode` | mode: int | - | Compositing operation (see `BlendMode`). |
//! | `SetLinearGradient` | target: int, x0: num, y0: num, x1: num, y1: num, stops: numbers | - | A linear gradient as fill (target 0) or stroke (target 1) style. |
//! | `SetRadialGradient` | target: int, x0: num, y0: num, r0: num, x1: num, y1: num, r1: num, stops: numbers | - | A radial gradient between two circles. |
//! | `SetConicGradient` | target: int, x: num, y: num, angle: num, stops: numbers | - | A conic (sweep) gradient starting at `angle` radians. |
//! | `SetPattern` | target: int, image: int, repeat: int | - | An image pattern; `repeat` 0 repeat, 1 repeat-x, 2 repeat-y, 3 no-repeat. |
//! | `SetShadow` | color: color, blur: num, dx: num, dy: num | - | Shadow colour, blur, and offset (alpha 0 turns it off). |
//! | `Translate` | x: num, y: num | - | Move the origin. |
//! | `Rotate` | angle: num | - | Rotate by radians. |
//! | `Scale` | x: num, y: num | - | Scale. |
//! | `Transform` | a: num, b: num, c: num, d: num, e: num, f: num | - | Multiply the transform by a matrix. |
//! | `SetTransform` | a: num, b: num, c: num, d: num, e: num, f: num | - | Replace the transform by a matrix. |
//! | `ResetTransform` | - | - | Back to the identity transform. |
//! | `SetFont` | css: text | - | The font, as a CSS font shorthand ("italic 700 16px Arial, sans-serif"). |
//! | `SetTextAlign` | align: int | - | 0 start, 1 end, 2 left, 3 right, 4 center. |
//! | `SetTextBaseline` | baseline: int | - | 0 alphabetic, 1 top, 2 hanging, 3 middle, 4 ideographic, 5 bottom. |
//! | `SetTextDirection` | direction: int | - | 0 auto, 1 left-to-right, 2 right-to-left. |
//! | `DrawText` | text: text, x: num, y: num, max_width: num, mode: int | - | One line of text; `max_width` 0 means no limit; `mode` bit 0 fills, bit 1 strokes. |
//! | `DrawTextBox` | text: text, x: num, y: num, width: num, line_height: num, mode: int | - | Text wrapped to `width` with `line_height` between lines. |
//! | `DrawImage` | image: int, x: num, y: num | - | An image at its natural size. |
//! | `DrawImageScaled` | image: int, x: num, y: num, w: num, h: num | - | An image scaled into a rectangle. |
//! | `DrawImageRegion` | image: int, sx: num, sy: num, sw: num, sh: num, dx: num, dy: num, dw: num, dh: num | - | A source rectangle of an image into a destination rectangle. |
//! | `DrawSprite` | image: int, frame: int, frame_w: num, frame_h: num, dx: num, dy: num, dw: num, dh: num | - | One frame of a sprite sheet (frames are laid out left to right, top to bottom). |
//!
//! ### Canvas3D (17 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `SetScene` | scene: handle | - | The Scene3D this canvas shows. |
//! | `SetCamera` | [camera: handle] | - | Look through this camera node (omit it, or give 0, for the scene's first camera, or one that frames everything). |
//! | `Connect` | - | text | Open the graphics device now (Render does it on first use); the answer is the Backend. |
//! | `GetBackend` | - | text | What runs the picture: WebGPU, WebGL2, Vulkan, Metal, Dx12, OpenGL, Software, or None. |
//! | `GetAdapter` | - | text | The graphics adapter's name and driver, when connected. |
//! | `GetStatus` | - | text | The readable message to show instead of the picture when 3D is not available (empty when it is). |
//! | `TakeEvent` | - | text | The next message the control should raise as onError (empty when there is none). |
//! | `Resize` | width: int, height: int | - | New size in pixels. |
//! | `GetWidth` | - | int | Width in pixels. |
//! | `GetHeight` | - | int | Height in pixels. |
//! | `SetMsaa` | samples: int | - | 1 (off) or 4 samples per pixel. |
//! | `GetMsaa` | - | int | Samples per pixel. |
//! | `SetShadowMapSize` | texels: int | - | The edge of each shadow map, 256 to 8192 (takes effect at the next Connect). |
//! | `Render` | - | int | Draw the scene from the camera; the answer is the number of draw calls. |
//! | `Screenshot` | - | handle | The last picture as a new ImageBuffer. |
//! | `Pick` | x: num, y: num | numbers | The node under a pixel: node (0 when none), instance, distance, point x y z, normal x y z. |
//! | `GetStats` | - | numbers | What the last frame held and drew: mesh nodes, instances, instances drawn, instances culled, triangles drawn, lights, shadow maps, skinned meshes, draw calls, shadow draw calls. |
//!
//! ### ImageBuffer (32 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `Create` | width: int, height: int, [color: color] | - | Make a blank image (transparent unless a colour is given). |
//! | `LoadAsset` | name: text | - | Load a bundled image by name (PNG, JPEG, GIF, WebP, BMP, QOI). |
//! | `LoadFromBytes` | bytes: bytes | - | Decode the bytes of an image file; the size limit is checked from its header before any pixel is made. |
//! | `Encode` | format: text, [quality: int] | bytes | The image as the bytes of a file in this format (quality 1 to 100 for JPEG). |
//! | `Save` | name: text, format: text, [quality: int] | int | Encode and hand the bytes to the host under this name; the answer is how many bytes. |
//! | `GetWidth` | - | int | Width in pixels. |
//! | `GetHeight` | - | int | Height in pixels. |
//! | `Clone` | - | handle | A copy as a new ImageBuffer (the pixels are shared until one of them changes). |
//! | `Resize` | width: int, height: int, [filter: text] | - | Resample to a new size: nearest, bilinear (default), catmull-rom or lanczos. |
//! | `Crop` | x: int, y: int, width: int, height: int | - | Keep a rectangle. |
//! | `Rotate` | degrees: num, [background: color] | - | Rotate clockwise (quarter turns are exact; other angles grow the canvas and fill the corners). |
//! | `Flip` | horizontal: flag | - | Mirror left-right (1) or top-bottom (0). |
//! | `Fill` | color: color | - | Set every pixel. |
//! | `GetPixel` | x: int, y: int | color | The pixel as 0xRRGGBBAA, or -1 outside the image. |
//! | `SetPixel` | x: int, y: int, color: color | int | Set one pixel; 1 if it was inside the image. |
//! | `DrawOnto` | canvas: handle, x: num, y: num, [width: num], [height: num] | - | Record a DrawImage of this image on a Canvas2D (scaled into width x height when given). |
//! | `Composite` | other: handle, x: int, y: int, [mode: text], [opacity: num] | - | Draw another image onto this one with a blend mode (source-over by default) and an opacity 0 to 1. |
//! | `Grayscale` | - | - | Turn to grey. |
//! | `Invert` | - | - | Invert the colours (alpha kept). |
//! | `Brightness` | amount: num | - | Add -1 to 1 (of full scale) to every colour channel. |
//! | `Contrast` | amount: num | - | Scale contrast about mid grey: 0 none, positive more, negative less. |
//! | `Hue` | degrees: num | - | Rotate the hue. |
//! | `Saturation` | factor: num | - | Multiply the saturation (0 grey, 1 unchanged, above 1 stronger). |
//! | `Threshold` | level: int | - | Black and white at a luma level 0 to 255. |
//! | `BlurGaussian` | sigma: num | - | Gaussian blur of this standard deviation in pixels. |
//! | `BlurBox` | radius: int | - | Box blur of this radius in pixels. |
//! | `Sharpen` | amount: num | - | Unsharp mask of this strength. |
//! | `EdgeDetect` | - | - | Sobel edges as a grey image. |
//! | `Convolve` | kernel: numbers, divisor: num, [offset: num] | - | Convolve with a square kernel (9, 25 ... numbers, row by row), then divide and add an offset. |
//! | `Dilate` | radius: int | - | Grow bright areas (maximum over a square window). |
//! | `Erode` | radius: int | - | Shrink bright areas (minimum over a square window). |
//! | `Histogram` | channel: int | ints | 256 counts for a channel: 0 red, 1 green, 2 blue, 3 alpha, 4 luma. |
//!
//! ### Plot (36 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `SetTitle` | text: text | - | The title above the chart. |
//! | `SetXLabel` | text: text | - | The label of the x axis. |
//! | `SetYLabel` | text: text | - | The label of the y axis. |
//! | `ShowLegend` | on: flag | - | Draw the legend of the named series (1) or not (0). |
//! | `ShowGrid` | on: flag | - | Draw grid lines (1) or not (0). |
//! | `SetBackground` | color: color | - | The background colour. |
//! | `SetForeground` | color: color | - | The colour of axes and text. |
//! | `SetFont` | family: text, [size: num] | - | The CSS font family of every label and its size in pixels. |
//! | `SetXRange` | min: num, max: num | - | Fix the x range (otherwise it fits the data). |
//! | `SetYRange` | min: num, max: num | - | Fix the y range (the z range of a 3D plot, the radius of a polar one). |
//! | `ClearRanges` | - | - | Back to fitting the data. |
//! | `AddCategory` | name: text | - | Name the next bar category (x = 0, 1, 2 ...). |
//! | `ClearCategories` | - | - | Forget the category names. |
//! | `SetHistogramBins` | bins: int | - | Bins of a histogram (0 chooses by Sturges' rule). |
//! | `SetView3D` | yaw: num, pitch: num | - | The viewing angles of a 3D plot in degrees. |
//! | `AddSeries` | kind: text, name: text | handle | A new series: LINE, BAR, SCATTER, AREA, PIE, HISTOGRAM, CANDLESTICK, HEATMAP, POLAR, ERRORBARS, SURFACE3D or SCATTER3D. The handle names it in later calls. |
//! | `RemoveSeries` | series: handle | - | Remove a series (its handle is released). |
//! | `ClearSeries` | series: handle | - | Empty a series, keeping its settings. |
//! | `GetSeriesCount` | - | int | How many series there are. |
//! | `GetSeriesPoints` | series: handle | int | How many points (values, candles, cells) a series holds. |
//! | `SetSeriesName` | series: handle, name: text | - | The legend text of a series. |
//! | `SetSeriesColor` | series: handle, color: color | - | The colour of a series (transparent takes one from the palette). |
//! | `SetSeriesLineWidth` | series: handle, width: int | - | Line width in pixels. |
//! | `SetSeriesPointSize` | series: handle, size: int | - | Marker radius in pixels (0 draws none on a line). |
//! | `SetSeriesClosed` | series: handle, closed: flag | - | Close a polar line back to its first point. |
//! | `AddPoint` | series: handle, x: num, y: num | - | Add an (x, y) point to a line, scatter, area or polar series (polar: angle in degrees, radius). |
//! | `AddPoint3` | series: handle, x: num, y: num, z: num | - | Add an (x, y, z) point to a 3D scatter series. |
//! | `AddValue` | series: handle, label: text, value: num | - | Add a value (with a label) to a bar, pie or histogram series. |
//! | `AddCandle` | series: handle, x: num, open: num, high: num, low: num, close: num | - | Add a candle to a candlestick series. |
//! | `AddErrorPoint` | series: handle, x: num, y: num, error: num | - | Add a point with a symmetric error to an error-bar series. |
//! | `SetSeriesGrid` | series: handle, columns: int, rows: int, values: numbers | - | Fill a heat map with a rows x columns grid of values, row by row. |
//! | `SetSeriesSurface` | series: handle, x0: num, x1: num, y0: num, y1: num, columns: int, rows: int, values: numbers | - | Fill a 3D surface: z over a rectangle, sampled on a rows x columns grid. |
//! | `SetSeriesFromTable` | series: handle, block: bytes, count: int, stride: int, columns: text | int | Fill a series from a COBOL table passed as one block: the row count, the bytes per row and where each number is and how it is stored, e.g. "0:zoned(6,0); 6:packed(7,2,s)". The answer is how many rows were skipped for holding an unreadable number. |
//! | `Render` | width: int, height: int | handle | Draw the chart into a new ImageBuffer. |
//! | `RenderToCanvas` | canvas: handle, x: num, y: num, width: int, height: int | - | Record the chart into a Canvas2D as vector commands, with its corner at (x, y). |
//! | `ToSvg` | width: int, height: int | text | The chart as SVG text. |
//!
//! ### Scene3D (71 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `AddGroup` | - | handle | An empty node to hang others from. |
//! | `AddBox` | width: num, height: num, depth: num | handle | A box centred on its node. |
//! | `AddSphere` | radius: num, [segments: int] | handle | A sphere (32 segments by default). |
//! | `AddPlane` | width: num, depth: num | handle | A flat rectangle in the XZ plane facing up. |
//! | `AddCylinder` | radius: num, height: num, [segments: int] | handle | A cylinder along Y. |
//! | `AddCone` | radius: num, height: num, [segments: int] | handle | A cone along Y, apex up. |
//! | `AddTorus` | radius: num, tube: num, [radial_segments: int], [tubular_segments: int] | handle | A torus in the XZ plane: radius to the middle of the tube, and the tube's own radius. |
//! | `AddMesh` | positions: numbers, indices: ints, [normals: numbers], [uvs: numbers] | handle | A mesh of your own: positions (x, y, z ...), triangle indices (three per triangle), optional normals (one per vertex; computed when omitted) and texture coordinates (u, v ...). |
//! | `LoadModel` | name: text | handle | Load a glTF 2.0 or GLB asset: meshes, materials, node transforms, skins and animations. The answer is the node that holds the whole model; what could not be honoured is in GetLoadWarnings. |
//! | `LoadModelFromBytes` | bytes: bytes | handle | Load a GLB (or a self-contained glTF) from bytes already in memory. |
//! | `GetLoadWarnings` | - | text | What the last model load could not honour (morph targets, unsupported primitives ...), one per line. |
//! | `AddDirectionalLight` | red: num, green: num, blue: num, intensity: num, [cast_shadow: flag] | handle | A sun along the node's -Z axis (aim it with LookAt); colour 0 to 1 per channel, intensity 1 lights a surface facing it with its own colour. |
//! | `AddPointLight` | red: num, green: num, blue: num, intensity: num, range: num | handle | A light in all directions; it falls off with the square of the distance and fades out at range (0 = never). |
//! | `AddSpotLight` | red: num, green: num, blue: num, intensity: num, range: num, inner_degrees: num, outer_degrees: num, [cast_shadow: flag] | handle | A cone of light along the node's -Z axis. |
//! | `SetLight` | node: handle, red: num, green: num, blue: num, intensity: num, range: num | - | Change a light's colour, intensity and range. |
//! | `SetAmbient` | red: num, green: num, blue: num, intensity: num | - | The flat light every surface gets from everywhere. |
//! | `AddCamera` | fov_degrees: num, near: num, far: num | handle | A perspective camera looking down its node's -Z axis; the field of view is vertical. |
//! | `AddOrthographicCamera` | height: num, near: num, far: num | handle | An orthographic camera; height is how much of the world fits vertically. |
//! | `SetBackground` | color: color | - | What the picture is cleared to (a colour with alpha 0 gives transparent pixels). |
//! | `SetBackgroundGradient` | top: color, bottom: color | - | A vertical gradient background. |
//! | `SetFog` | red: num, green: num, blue: num, near: num, far: num | - | Linear fog: none at near, total at far. |
//! | `SetFogExp` | red: num, green: num, blue: num, density: num | - | Exponential fog: 1 - exp(-density x distance). |
//! | `ClearFog` | - | - | No fog. |
//! | `SetParent` | node: handle, [parent: handle] | - | Put a node under another (omit the parent, or give 0, to make it a root); its local transform is kept. |
//! | `GetParent` | node: handle | handle | A node's parent (0 for a root). |
//! | `GetChildren` | node: handle | ints | A node's children. |
//! | `Remove` | node: handle | - | Remove a node and everything under it. |
//! | `SetName` | node: handle, name: text | - | Name a node. |
//! | `GetName` | node: handle | text | A node's name. |
//! | `FindNode` | name: text | handle | The first node with this name (0 if none). |
//! | `SetVisible` | node: handle, visible: flag | - | Show or hide a node and everything under it. |
//! | `IsVisible` | node: handle | int | 1 if the node itself is visible. |
//! | `GetNodeKind` | node: handle | text | GROUP, MESH, LIGHT or CAMERA. |
//! | `SetPosition` | node: handle, x: num, y: num, z: num | - | Position relative to the parent. |
//! | `SetRotation` | node: handle, x_degrees: num, y_degrees: num, z_degrees: num | - | Rotation as Euler angles in degrees: yaw about Y, then pitch about X, then roll about Z. |
//! | `SetRotationQuaternion` | node: handle, x: num, y: num, z: num, w: num | - | Rotation as a quaternion. |
//! | `SetScale` | node: handle, x: num, y: num, z: num | - | Scale on each axis (zero is refused). |
//! | `LookAt` | node: handle, target_x: num, target_y: num, target_z: num, [up_x: num], [up_y: num], [up_z: num] | - | Turn a node so its -Z axis points at a target (up defaults to +Y). |
//! | `GetPosition` | node: handle | numbers | x, y, z relative to the parent. |
//! | `GetRotation` | node: handle | numbers | Euler degrees as SetRotation takes them. |
//! | `GetRotationQuaternion` | node: handle | numbers | x, y, z, w. |
//! | `GetScale` | node: handle | numbers | x, y, z. |
//! | `GetWorldPosition` | node: handle | numbers | x, y, z in the scene (not relative to the parent). |
//! | `CreateMaterial` | - | handle | A new material: white, roughness 0.5, not metallic. |
//! | `ReleaseMaterial` | material: handle | - | Release a material; nodes that used it fall back to the default. |
//! | `SetMaterial` | node: handle, [material: handle] | - | Draw a node with this material (omit it, or give 0, for the default). |
//! | `GetMaterial` | node: handle | handle | The material a mesh node uses; a node that had none is given a new one. |
//! | `SetColor` | node: handle, color: color | - | Shortcut: the base colour (and opacity) of a mesh node's own material. |
//! | `SetMaterialColor` | material: handle, color: color | - | A material's base colour and opacity. |
//! | `SetMetallic` | material: handle, value: num | - | 0 for plastic or wood, 1 for a metal. |
//! | `SetRoughness` | material: handle, value: num | - | 0 mirror-smooth to 1 fully rough. |
//! | `SetEmissive` | material: handle, red: num, green: num, blue: num | - | Light the surface gives off. |
//! | `SetTexture` | material: handle, slot: int, [image: handle] | - | An ImageBuffer on a slot (0 base colour, 1 metallic-roughness, 2 emissive, 3 normal); omit the image, or give 0, to clear the slot. The pixels are shared, not copied. |
//! | `SetAlphaMode` | material: handle, mode: int, [cutoff: num] | - | 0 opaque, 1 mask (pixels under the cutoff are dropped), 2 blend. |
//! | `SetDoubleSided` | material: handle, on: flag | - | Draw both sides of every triangle. |
//! | `SetUnlit` | material: handle, on: flag | - | Ignore the lights: draw the colour as it is. |
//! | `SetInstances` | node: handle, matrices: numbers | - | Draw the node's mesh once per matrix (sixteen numbers each, column-major with the translation in the last column); an empty list goes back to one copy. |
//! | `GetInstanceCount` | node: handle | int | How many copies a node draws. |
//! | `GetAnimations` | - | ints | Every animation. |
//! | `FindAnimation` | name: text | handle | An animation by name (0 if none). |
//! | `GetAnimationName` | animation: handle | text | The name an animation has in its model. |
//! | `GetAnimationDuration` | animation: handle | num | Seconds. |
//! | `GetAnimationTime` | animation: handle | num | The current time in seconds. |
//! | `IsAnimationPlaying` | animation: handle | int | 1 if Update advances it. |
//! | `PlayAnimation` | animation: handle, [loop_it: flag], [speed: num] | - | Start (or resume) an animation; it loops by default; speed 1 is real time, negative runs backwards. |
//! | `StopAnimation` | animation: handle | - | Pause an animation where it is. |
//! | `SetAnimationTime` | animation: handle, seconds: num | - | Jump to a time and show that pose. |
//! | `Update` | milliseconds: num | - | Advance every playing animation by the elapsed milliseconds (what onFrame gives). |
//! | `GetNodeCount` | - | int | How many nodes. |
//! | `GetTriangleCount` | - | int | How many triangles the meshes hold (instances not counted). |
//! | `GetBounds` | - | numbers | valid (0 for an empty scene), then min x y z and max x y z of everything drawn. |
//!
//! ### Physics2D (40 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `CreateWorld` | gx: num, gy: num | handle | A new world with this gravity (metres per second squared; 0 -9.81 is Earth with y up). |
//! | `DestroyWorld` | world: handle | - | Release a world and everything in it. |
//! | `SetGravity` | world: handle, gx: num, gy: num | - | Change the gravity. |
//! | `SetFixedStep` | world: handle, seconds: num, [max_substeps: int] | - | Step in equal sub-steps of this many seconds (0 for none), at most max_substeps (default 8) per Step call. |
//! | `SetThreads` | world: handle, threads: int | - | Pin the solver to this many threads (0: the default pool). The results do not depend on it. |
//! | `AddBody` | world: handle, kind: int, x: num, y: num, [angle: num] | handle | A body: kind 0 static, 1 dynamic, 2 kinematic (moved by its velocity); the angle is in radians. |
//! | `RemoveBody` | world: handle, body: handle | - | Remove a body, its shapes and its joints. |
//! | `AddBall` | world: handle, body: handle, radius: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A disc on a body. |
//! | `AddBox` | world: handle, body: handle, hx: num, hy: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A rectangle with these half extents on a body. |
//! | `AddCapsule` | world: handle, body: handle, half_height: num, radius: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A capsule along Y on a body. |
//! | `AddConvexHull` | world: handle, body: handle, points: numbers, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | The convex hull of points (x, y ...) on a body. |
//! | `RemoveShape` | world: handle, shape: handle | - | Remove one shape. |
//! | `SetPosition` | world: handle, body: handle, x: num, y: num | - | Teleport a body. |
//! | `SetRotation` | world: handle, body: handle, angle: num | - | Set the angle in radians (counter-clockwise). |
//! | `SetVelocity` | world: handle, body: handle, x: num, y: num | - | Set the velocity. |
//! | `SetAngularVelocity` | world: handle, body: handle, radians_per_second: num | - | Set the angular velocity. |
//! | `ApplyForce` | world: handle, body: handle, x: num, y: num | - | A force (newtons) that acts during the next Step call. |
//! | `ApplyTorque` | world: handle, body: handle, torque: num | - | A torque that acts during the next Step call. |
//! | `ApplyImpulse` | world: handle, body: handle, x: num, y: num | - | Change the velocity at once by impulse / mass. |
//! | `GetPosition` | world: handle, body: handle | numbers | x, y. |
//! | `GetRotation` | world: handle, body: handle | num | The angle in radians. |
//! | `GetVelocity` | world: handle, body: handle | numbers | x, y. |
//! | `GetAngularVelocity` | world: handle, body: handle | num | Radians per second. |
//! | `IsSleeping` | world: handle, body: handle | int | 1 if the solver has put the body to sleep. |
//! | `AddFixedJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, bx: num, by: num | handle | Weld two bodies at an anchor given in each body's own space. |
//! | `AddRevoluteJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, bx: num, by: num, [min_angle: num], [max_angle: num] | handle | A hinge, optionally limited to an angle range in radians. |
//! | `AddPrismaticJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, bx: num, by: num, axis_x: num, axis_y: num, [min: num], [max: num] | handle | A slider along an axis, optionally limited to a distance range. |
//! | `RemoveJoint` | world: handle, joint: handle | - | Remove a joint. |
//! | `RayCast` | world: handle, ox: num, oy: num, dx: num, dy: num, max_distance: num | numbers | The first shape the ray meets: hit (1 or 0), body, shape, distance, point x y, normal x y. The direction need not be a unit vector. |
//! | `Step` | world: handle, seconds: num | int | Advance by this many seconds (at most 1); with a fixed step it runs whole sub-steps. Bound Scene3D nodes follow their bodies. The answer is how many steps ran. |
//! | `TakeEvents` | world: handle | numbers | The collision events since the last call, six numbers each: 0 started or 1 stopped, body a, body b, shape a, shape b, 1 if a sensor was involved. |
//! | `BindSprite` | world: handle, body: handle, sprite: int | - | Make a body drive a sprite the host knows by this number (read it back with GetBound). |
//! | `BindNode` | world: handle, body: handle, scene: handle, node: handle | - | Make a body drive a Scene3D node (z = 0, turning about Z): after every Step the node is where the body is. |
//! | `Unbind` | world: handle, body: handle | - | Stop a body driving anything. |
//! | `GetBound` | world: handle | numbers | The bound bodies, seven numbers each: body, 0 for a sprite or 1 for a node, the sprite number (or the scene handle), 0 (or the node handle), x, y, angle. |
//! | `GetBodyCount` | world: handle | int | Bodies in a world. |
//! | `GetShapeCount` | world: handle | int | Shapes in a world. |
//! | `GetJointCount` | world: handle | int | Joints in a world. |
//! | `GetWorldCount` | - | int | Worlds in this object. |
//! | `GetStateFingerprint` | world: handle | int | A number that is the same for two runs that ended in exactly the same state (positions, angles, velocities, to the bit). |
//!
//! ### Physics3D (42 methods)
//!
//! | Method | Parameters | Returns | What it does |
//! |---|---|---|---|
//! | `CreateWorld` | gx: num, gy: num, gz: num | handle | A new world with this gravity (metres per second squared; 0 -9.81 0 is Earth). |
//! | `DestroyWorld` | world: handle | - | Release a world and everything in it. |
//! | `SetGravity` | world: handle, gx: num, gy: num, gz: num | - | Change the gravity. |
//! | `SetFixedStep` | world: handle, seconds: num, [max_substeps: int] | - | Step in equal sub-steps of this many seconds (0 for none), at most max_substeps (default 8) per Step call. |
//! | `SetThreads` | world: handle, threads: int | - | Pin the solver to this many threads (0: the default pool). The results do not depend on it. |
//! | `AddBody` | world: handle, kind: int, x: num, y: num, z: num | handle | A body: kind 0 static, 1 dynamic, 2 kinematic (moved by its velocity). |
//! | `RemoveBody` | world: handle, body: handle | - | Remove a body, its shapes and its joints. |
//! | `AddBall` | world: handle, body: handle, radius: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A sphere on a body. |
//! | `AddBox` | world: handle, body: handle, hx: num, hy: num, hz: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A box with these half extents on a body. |
//! | `AddCapsule` | world: handle, body: handle, half_height: num, radius: num, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A capsule along Y on a body. |
//! | `AddConvexHull` | world: handle, body: handle, points: numbers, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | The convex hull of points (x, y, z ...) on a body. |
//! | `AddTrimesh` | world: handle, body: handle, vertices: numbers, triangles: ints, [density: num], [friction: num], [restitution: num], [sensor: flag] | handle | A triangle mesh on a body (best on a static one): vertices (x, y, z ...) and triangles as indices, three per triangle. |
//! | `RemoveShape` | world: handle, shape: handle | - | Remove one shape. |
//! | `SetPosition` | world: handle, body: handle, x: num, y: num, z: num | - | Teleport a body. |
//! | `SetRotation` | world: handle, body: handle, x: num, y: num, z: num, w: num | - | Set the orientation as a quaternion. |
//! | `SetRotationEuler` | world: handle, body: handle, x_degrees: num, y_degrees: num, z_degrees: num | - | Set the orientation from Euler angles in degrees (yaw about Y, then pitch about X, then roll about Z, as Scene3D's SetRotation). |
//! | `SetVelocity` | world: handle, body: handle, x: num, y: num, z: num | - | Set the velocity. |
//! | `SetAngularVelocity` | world: handle, body: handle, x: num, y: num, z: num | - | Set the angular velocity (radians per second about each axis). |
//! | `ApplyForce` | world: handle, body: handle, x: num, y: num, z: num | - | A force (newtons) that acts during the next Step call. |
//! | `ApplyTorque` | world: handle, body: handle, x: num, y: num, z: num | - | A torque that acts during the next Step call. |
//! | `ApplyImpulse` | world: handle, body: handle, x: num, y: num, z: num | - | Change the velocity at once by impulse / mass. |
//! | `GetPosition` | world: handle, body: handle | numbers | x, y, z. |
//! | `GetRotation` | world: handle, body: handle | numbers | The orientation as a quaternion x, y, z, w. |
//! | `GetVelocity` | world: handle, body: handle | numbers | x, y, z. |
//! | `GetAngularVelocity` | world: handle, body: handle | numbers | x, y, z. |
//! | `IsSleeping` | world: handle, body: handle | int | 1 if the solver has put the body to sleep. |
//! | `AddFixedJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, az: num, bx: num, by: num, bz: num | handle | Weld two bodies at an anchor given in each body's own space. |
//! | `AddRevoluteJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, az: num, bx: num, by: num, bz: num, axis_x: num, axis_y: num, axis_z: num, [min_angle: num], [max_angle: num] | handle | A hinge about an axis, optionally limited to an angle range in radians. |
//! | `AddPrismaticJoint` | world: handle, body_a: handle, body_b: handle, ax: num, ay: num, az: num, bx: num, by: num, bz: num, axis_x: num, axis_y: num, axis_z: num, [min: num], [max: num] | handle | A slider along an axis, optionally limited to a distance range. |
//! | `RemoveJoint` | world: handle, joint: handle | - | Remove a joint. |
//! | `RayCast` | world: handle, ox: num, oy: num, oz: num, dx: num, dy: num, dz: num, max_distance: num | numbers | The first shape the ray meets: hit (1 or 0), body, shape, distance, point x y z, normal x y z. The direction need not be a unit vector. |
//! | `Step` | world: handle, seconds: num | int | Advance by this many seconds (at most 1); with a fixed step it runs whole sub-steps. Bound Scene3D nodes follow their bodies. The answer is how many steps ran. |
//! | `TakeEvents` | world: handle | numbers | The collision events since the last call, six numbers each: 0 started or 1 stopped, body a, body b, shape a, shape b, 1 if a sensor was involved. |
//! | `BindSprite` | world: handle, body: handle, sprite: int | - | Make a body drive a sprite the host knows by this number (read it back with GetBound). |
//! | `BindNode` | world: handle, body: handle, scene: handle, node: handle | - | Make a body drive a Scene3D node: after every Step the node is where the body is. |
//! | `Unbind` | world: handle, body: handle | - | Stop a body driving anything. |
//! | `GetBound` | world: handle | numbers | The bound bodies, eleven numbers each: body, 0 for a sprite or 1 for a node, the sprite number (or the scene handle), 0 (or the node handle), position x y z, rotation x y z w. |
//! | `GetBodyCount` | world: handle | int | Bodies in a world. |
//! | `GetShapeCount` | world: handle | int | Shapes in a world. |
//! | `GetJointCount` | world: handle | int | Joints in a world. |
//! | `GetWorldCount` | - | int | Worlds in this object. |
//! | `GetStateFingerprint` | world: handle | int | A number that is the same for two runs that ended in exactly the same state (positions, rotations, velocities, to the bit). |
//!
//! <!-- methods:end -->
//!
//! # Differences from the HTML canvas
//!
//! See the table in [`canvas`].

pub(crate) mod api;
pub mod canvas;
pub mod canvas2d;
pub mod color;
pub(crate) mod detmath;
pub mod display_list;
pub mod error;
pub mod handles;
#[cfg(feature = "imagebuf")]
pub mod imagebuf;
pub mod limits;
pub mod par;
pub mod pixels;
#[cfg(feature = "physics")]
pub mod physics;
#[cfg(feature = "plot")]
pub mod plot;
pub mod registry;
pub mod resources;
#[cfg(feature = "scene3d-core")]
pub mod scene3d;
#[cfg(feature = "text")]
pub mod text;
pub mod textapi;
pub mod value;

pub use canvas2d::Canvas2D;
pub use color::Rgba;
pub use error::{GraphicsError, Result};
pub use limits::Limits;
pub use pixels::PixelBuffer;
pub use registry::{MethodInfo, ObjectKind, Owner, Registry};
pub use value::Value;
