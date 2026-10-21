<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Control reuse on the web (WASM-004)

- **Status:** the player half (classes A, B, E-by-name) is implemented and tested; the IDE half (toolbox, Check, Properties, KB tables) and the desktop-versus-browser comparison harnesses are not
- **Spec:** ./spec.md   **Date:** 2026-10-20

Tools and browser as in WASM-016's plan (rustc 1.95.0, `nightly-2026-10-08` for the threaded variant only, `wasm-bindgen` 0.2.129, `wasm-opt` 133, node v23.3.0, Chrome 152.0.7977.149).

## 1. Approach

The player draws a control from **the same `.cfrm` control** the desktop uses (R1): `FormTree::build` reads the form with `cobolt_forms`, keeps each control's properties exactly as the model stores them, and the view layer turns `(tree, page state)` into `VNode` data.

- **R2 — one table.** `crates/cobolt-web-player/src/kinds.rs` lists every `ControlType::ALL` entry once with its class (A-E), what it becomes in the browser and a note where operations differ. The controls the model does not carry yet (WebLink, Authentication, Canvas2D, Canvas3D, the five E objects) arrive as `ControlType::Custom` and are recognised by name (`player_kind`). A test fails when the model and the table disagree (AC1).
- **R3, class A.** The native element that fits: `<button>`, `<input>` / `<textarea>`, `<select>`, `<progress>`, `<a>` for WebLink, `<dialog>` for modal forms, and so on; the browser supplies focus, selection, IME and accessibility. TabOrder becomes the element's `tabindex` (stop types only; a disabled control stays out).
- **R4, class B.** SVG, `<img>` or `<canvas>` inside the control's own element: the seven charts (SVG built by `view/charts.rs`), Maps (tiles as `<img crossorigin="anonymous">`, pan by drag, zoom by wheel / buttons / keys, bounds event), Knob / Gauge / Switch / Slider, Shape / Line, Animator (the browser plays the formats), Viewer (below), Canvas2D (below).
- **Viewer.** Markdown, HTML subset, text, **Mermaid** (the Mermaid part returns SVG text, read by an allow-listed SVG reader that builds nodes: no markup is ever inserted), **PDF** (the PDF part returns each page's size and text; the page shows them), and **Find** (spec 058 R26-R30): a bar, a live counter, next / previous with wrap, match case, Ctrl/Cmd+F and Escape; highlights with the CSS Custom Highlight API so no node is split or wrapped.
- **TextBox Picture** (numeric-edited and alphanumeric-edited input): the box shows the edited form at rest and the raw digits while focused; keystrokes that cannot belong to the picture are refused (the stored text is written back); the sign is normalised on blur. The decimal comma and the currency sign come from the form's `SPECIAL-NAMES`. The engine is `cobolt_forms::picture`, shared with the desktop.
- **FileDropZone**: a real drop target (`dragover`/`drop`, plus the file picker): classification by extension and size is `view::interact::classify_files`; the files stay in the page, the program sees their names and `onFilesDropped`.
- **Canvas2D** (WASM-018 R7, integration in the player): the control is a `<canvas>`; its drawing methods (`FillRect`, `SetFillColor`, `DrawText`, `Present`, ...) are recorded per control, in order, and run by the **Canvas part**, which holds `cobolt-graphics`' own `Canvas2D`; when a `Present` has run the picture is put on the element with `putImageData`.
  Read-back methods (`GetPixel`, `MeasureText`, `IsPointInPath`, `ToImage`, `Capture`) are refused with the reason: they need an answer at once and the part loads asynchronously. Canvas3D is not drawn (no 3D part exists yet).
- **R8 — layout is CSS.** `layout_css.rs` maps the four layout modes and the breakpoints to CSS; the Rust solver does not run in the browser.
- **R9, R10 — touch and keyboard.** Pointer events (one code path for mouse, touch and pen); native elements keep their roles; custom ones set `role`, `aria-*` and `tabindex` in the view, with tests on the produced attributes.

## 2. Affected crates / files

- `crates/cobolt-web-player/src/{kinds,tree,methods,css,layout_css,base_css}.rs`, `src/view/*` (one view and one interaction function per control), `src/dom/{apply,find,canvas}.rs`, `parts/{mermaid,pdf,canvas}`.
- `crates/cobolt-forms/` — `picture`, `viewer` (`find_matches`, `parse_pdf`, `render_mermaid_svg`), `dropzone`, `map_geometry`, `datagrid`: the pure logic the player reuses unchanged.
- Not here: `crates/cobolt-ide` (toolbox, Properties, Check), `crates/cobolt-compiler` (the System KB tables, R12), `tests/controls/`.

## 3. Data / model changes

None to `.cfrm`. Properties with no effect on the web are kept and ignored (Q3), so no data is lost on a round trip. Web-only members (R7) were not added: nothing in the player invented a property or method the model lacks, except the Viewer's Find members, which are spec 058's own (`View1FindOpen`, `View1SearchText`, `View1SearchCurrentMatch`, `View1SearchCaseSensitive`, `FindNext`, `FindPrevious`, `FindOpen`, `FindClose`).

## 4. Key decisions & alternatives

- **Decision: the view is data (`VNode`), the page is a patcher.** Why: every control's DOM is asserted natively (no browser) and the same data drives the node harness. Rejected: building nodes inside each control's code.
- **Decision: refuse invalid keystrokes by writing the stored text back (`quiet`)**, not by `preventDefault` on `beforeinput`: IME, paste and drag-drop all reach the same check, and the box never shows a value the picture would reject.
- **Decision: page-only state is applied the moment the event arrives**, not when the driver next takes the inbox in. Back-to-back pointer events of a drag outran the inbox, so the map's drag origin was recorded after the moves that needed it (found by driving a real drag in Chrome; fixed and re-verified).
- **Decision: Maps tiles use `crossorigin="anonymous"`** so they load under COEP `require-corp`; 6 of 6 tiles loaded on an isolated page.
- **Decision: PDF text and size only, no page painting.** The desktop Viewer also paints a PDF's lines and rectangles (`pdf_paint`, behind the `render` feature with egui); the browser shows what a reader and a screen reader use. A page-painting path through the Canvas part is possible later.
- **Decision: the 62 controls are not all class A/B.** Class C, D and E controls follow the table (WASM-004 §2); what the player does for a class C or D control on a form (R5's Check is the IDE's) was not exercised as a whole.

## 5. Risks & mitigations

- Risk: a control behaves differently on the desktop and in the browser. → The per-control tests assert the DOM and the event order; the desktop comparison (AC3, AC6) is **not run** (no shared harness exists yet).
- Risk: accessibility. → Roles and names are asserted in the view tests; **no automated accessibility audit was run** (AC7).
- Risk: web-only features missing in an old browser (the Custom Highlight API). → Find degrades to count, current match and scrolling without colour.

## 6. Test strategy

- `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib` — 220 tests including: the table-versus-model test (AC1), a view test per control, the interaction tests (keys, pointer, Find, Picture, FileDropZone, map pan/zoom, knob), `methods` (list, numeric stepper, text, Canvas2D) and `viewer` (Markdown, Mermaid, PDF, Find bar).
- Chrome 152 (gallery bundle, 127.0.0.1 only):
  - **TextBox Picture**: `ZZ,ZZ9.99-` shows the edited form at rest and the raw digits focused; typing `A` into a numeric picture is refused; the sign is normalised on blur.
  - **Find**: counter `1 / 14`, next and previous wrap, the program's `FindNext`, match case (12), no `<mark>` created, highlights cleared when the bar closes.
  - **WebLink**: `<a href target=_blank rel="noopener noreferrer">`, no `ping`, `javascript:` refused by `SetUrl`, `mailto:` accepted, a plain click not prevented.
  - **Map**: pan (a synthetic drag moved the tiles by the drag's distance), zoom in and out by the buttons, 6/6 tiles loaded.
  - **Knob**: a drag from the right to the top moved the value 35 -> 50.
  - **FileDropZone**: a dragged-over highlight, a drop of `a.txt` listed in the control.
  - **PDF**: a hand-built one-page PDF showed `Page 1 of 1 (612 x 792 pt)` and its two lines; the PDF part was fetched on first use only.
  - **Mermaid**: the diagram of the gallery document drawn from SVG text.
  - **Canvas2D**: see the node scenario `parts: the Canvas 2D part runs a display list` (2.8 ms to draw and read back an 8x8 picture) and the browser run recorded in tasks.md.
- Measured sizes: Mermaid part 1,906,933 bytes, PDF part 219,273, Canvas part 469,419 (see WASM-016 plan).

## 7. Steering compliance

- [x] COBOL identifiers English; no "cobolt" in user-facing text.
- [x] No invented property or method; Find's members are the model's own.
- [ ] System KB tables (R12) and `chunked.data`: **not touched** (the compiler's doc tables are the IDE side).
- [ ] i18n: the player's few visible texts (the Find bar's, the dialogs') are English; the IDE's `Tr` keys are the IDE side.
- [x] `README.md` and `docs/` untouched.
