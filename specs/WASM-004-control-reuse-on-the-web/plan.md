<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Plan — Control reuse on the web (WASM-004)

- **Status:** revised 2026-10-22 to match the spec at 2.0.29 (the operator's answers of 2026-10-22, the form layers R13–R24). The player half of R2–R4 and R8–R10 is implemented and tested; the class table's move to one shared place, the layers, the IDE half (toolbox, Check, Properties, KB tables), the decided fonts and touch rules and the desktop-versus-browser harnesses are **not**. Waits for `/tasks`.
- **Spec:** ./spec.md   **Date:** 2026-10-22 (first written 2026-10-20)

Tools and browser as in WASM-016's plan (rustc 1.95.0, `nightly-2026-10-08` for the threaded variant only, `wasm-bindgen` 0.2.129, `wasm-opt` 133, node v23.3.0, Chrome 152.0.7977.149).

## 0. What changed in this revision, and why

| # | Change | Why (spec text it follows) | Code read |
|---|---|---|---|
| C1 | **The class table moves from the player into `cobolt-forms`** (a `web` module, no egui), and the player reads it | R2 requires *one place the toolbox, Check, the KB and the agent kit all read*. Today the only table is `crates/cobolt-web-player/src/kinds.rs::TABLE`; `cobolt-ide` and `cobolt-compiler` depend on `cobolt-forms` but not on the player (`crates/cobolt-ide/Cargo.toml`, `crates/cobolt-compiler/Cargo.toml`), so none of them can read it | `kinds.rs` (`TABLE`, `class_of`, `PlayerKind`), both `Cargo.toml`s |
| C2 | **R6: a property with no use on the web is hidden** in the Properties panel of a web or mobile form (it was "kept, ignored, marked") | Q3 decided by the operator 2026-10-22. The value still round-trips (AC5) | `crates/cobolt-web-player/src/css.rs::TABLE` (`Fidelity::NotOnWeb`), `panels/properties.rs::show_selection` |
| C3 | **The fidelity column moves with the class table** and is re-examined against Q6 | Q6 decided: *reproduce the look in CSS*. Several rows marked `NotOnWeb`/`Approximate` (`BorderGradientEnabled`, `BorderBlur`, `BorderTransparency`, the shadow rows) describe a look CSS can draw; the row is the input of R6's hide rule, so a row left wrong would hide a property the operator wants reproduced | `css.rs::TABLE` rows 74–98 |
| C4 | **Form layers on the web, R13–R24, are planned** (§1.4) | Added to the spec at 2.0.26; the earlier plan predates them. The player has no layer code at all (no match for "layer" outside `kinds.rs` doc text), and the ABI manifest has no `layers` field (`crates/cobolt-wasm-abi/src/manifest.rs::FormEntry`), so ABI amendment A9 is unimplemented on both sides | `tree.rs::FormTree::build`, `view/style.rs`, `base_css.rs`, `manifest.rs` |
| C5 | **Tab order: DOM order and `tabindex="0"`, never a positive `tabindex`** | R20 forbids positive values. The player today writes `tabindex="n"` from `TabOrder` (`view/controls.rs::with_tab_order`, asserted by the test at `controls.rs:873` expecting `"3"`); that implementation and its test change | `view/controls.rs` |
| C6 | **The non-visual predicate: one source, a test of equality** | R21: the player's copy "shall stay equal to it, as its test requires". `kinds.rs::is_non_visual` repeats `model.rs::ControlType::is_non_visual`'s list, and its test checks four cases, not equality | `kinds.rs::is_non_visual`, `model.rs:4152` |
| C7 | **Fonts are bundled** (§1.6) | Q5 decided: bundle; the Build warns above 50 % of the application. The player uses the system stack today (`base_css.rs`, `--pr-font: system-ui, …`) | `base_css.rs`, `crates/cobolt-forms/src/fonts.rs`, `crates/cobolt-web-bundle/src/lib.rs` (`warnings`) |
| C8 | **Touch hit area 15 % larger** (§1.7) | Q9 decided | `view/controls.rs::root` users |
| C9 | **Layer opacity from the description**, shared with the desktop | Q7 decided (first option). The desktop's rule exists as a private `render.rs::layer_is_opaque` behind the `render` feature | `crates/cobolt-forms/src/render.rs:435` |
| C10 | **Snackbar's toast above every layer**; DataGrid's remote tables paged on the server | Q8, Q4 decided | `kinds.rs` row `Snackbar`; row `DataGrid` |
| C11 | **Hyperlink** is the control's name | The spec (and WASM-006) renamed WebLink → Hyperlink at 2.0.29. The player still says `WebLink` (`kinds.rs::PLAYER_KINDS`, `weblink.rs`). The rename is WASM-006's work; this plan's table uses the new name and depends on it | `kinds.rs`, `weblink.rs` |
| C12 | Q2 stays **open**; the working assumption is written down (§4) | The operator's reply of 2026-10-22 does not answer it | — |

Kept unchanged from 2026-10-20: §1.1–§1.3 below (the player's control realisation, Viewer, Picture, FileDropZone, Canvas2D, CSS layout), the decisions on `VNode` data, keystroke refusal, page-only state, map tiles and PDF text, and every Chrome run recorded in §6.

## 1. Approach

### 1.1 One definition (R1, R2, R7) — **revised**

The player draws a control from **the same model control** the desktop uses (R1): `FormTree::build` (`crates/cobolt-web-player/src/tree.rs`) reads the form with `cobolt_forms`, keeps each control's properties exactly as the model stores them, and the view layer turns `(tree, page state)` into `VNode` data. A web form is a `.wcfrm`, a mobile form a `.mcfrm` (WASM-001 Q2): the same model, another extension — this spec adds no field for it.

- **The table (R2) lives in `cobolt-forms`** — new module `crates/cobolt-forms/src/web.rs`, compiled without the `render` feature (the player already depends on `cobolt-forms` with `default-features = false`). It holds, once:
  - `WebClass { A, B, C, D, E }` and one row per `ControlType::ALL` entry: class, what it becomes in the browser, and the **operations that take another class** as data (`&[(operation, WebClass)]`, named as `crates/cobolt-ast/src/methods.rs::is_known_method` spells them, e.g. Maps: `Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix` → C; Viewer: opening and saving a file (`SaveAs`, `SaveAsPdf`, and the open path) → C, `Print`, `Share` → D; ToolBar: the actions that start a program → D; the exact list per control is fixed in `/tasks` against `methods.rs`, no name invented), so Check can name the operation (R2, R5).
  - The series controls the model does not carry yet (Hyperlink, Authentication, Canvas2D, Canvas3D, ImageBuffer, Plot, Scene3D, Physics2D, Physics3D) keep arriving as `ControlType::Custom` and are recognised by name, as `kinds.rs::player_kind_of` does today; the name list moves with the table. When WASM-006/009/018 add real variants, their rows move from the by-name list to `ALL` and the AC1 test keeps both honest.
  - The **property-fidelity rows** (property → `Exact | Approximate | NotOnWeb`, note) that `css.rs::TABLE` carries today. The CSS strings and the functions that compute declarations stay in the player's `css.rs`, keyed by the shared rows; a test fails when a property is in one and not the other.
- **The player** keeps `kinds.rs` as a thin reader (`class_of`, `PlayerKind`), so its call sites (`host.rs`, `view/style.rs`) do not change shape.
- **The non-visual set (R21)**: `kinds.rs::is_non_visual` stops repeating the list: for model types it calls `ControlType::is_non_visual()`; for the by-name kinds it keeps `!PlayerKind::visual()`. A test iterates `ControlType::ALL` and asserts equality, and asserts the E objects are non-visual (they join the model predicate when WASM-018 adds them).
- **Web-only members (R7).** None exists yet. The two the spec names (`CssClass`, `Style`) are WASM-007's; they are added to the model (`Control::new` seeding, `runtime_property_names_for`), to `crates/cobolt-ast/src/methods.rs` (`is_known_method` where a method) and to the KB tables in WASM-007's change, as R1 says. This plan adds a guard test: every property the player's view reads by name (`cx.tree.get(i, "…")`) is a model property or a layer property (`model.rs::LAYER_PROPS`), so a web-only member cannot appear by accident.

### 1.2 Behaviour of classes A, B, E (R3, R4, R9, R10) — kept

- **Class A.** The native element that fits: `<button>`, `<input>` / `<textarea>`, `<select>`, `<progress>`, `<a>` for Hyperlink, `<dialog>` for modal forms, and so on; the browser supplies focus, selection, IME and accessibility.
- **Class B.** SVG, `<img>` or `<canvas>` inside the control's own element: the seven charts (SVG built by `view/charts.rs`), Maps (tiles as `<img crossorigin="anonymous">`, pan by drag, zoom by wheel / buttons / keys, bounds event), Knob / Gauge / Switch / Slider, Shape / Line, Animator (the browser plays the formats), Viewer, Canvas2D.
- **Viewer.** Markdown, HTML subset, text, **Mermaid** (SVG text read by an allow-listed SVG reader that builds nodes: no markup is ever inserted), **PDF** (each page's size and text), and **Find** (spec 058 R26–R30) with the CSS Custom Highlight API.
- **TextBox Picture**: the edited form at rest, the raw digits while focused; invalid keystrokes refused; the sign normalised on blur; decimal comma and currency sign from `SPECIAL-NAMES`; engine `cobolt_forms::picture`, shared with the desktop.
- **FileDropZone**: a real drop target (`dragover`/`drop`, plus the file picker); classification `view::interact::classify_files`.
- **Canvas2D** (WASM-018 R7): a `<canvas>`; drawing methods recorded per control and run by the Canvas part (`cobolt-graphics`' `Canvas2D`), the picture put with `putImageData` after `Present`. Read-backs (`GetPixel`, `MeasureText`, `IsPointInPath`, `ToImage`, `Capture`) are refused with the reason; Canvas3D is not drawn (no 3D part). Both stay open items of WASM-018 (tasks T11).
- **R6 / Q6 — the look in CSS.** The operator decided to reproduce the egui look (Knob, glass shadows, spec 057's drawn corners) in CSS. The CSS itself is WASM-007's (`css.rs`); what this spec does is re-mark each fidelity row from what the CSS actually achieves after that work (C3) and let the row drive the Properties panel (§1.5). Spatial's operating-system blur (`OsBlur`) stays `NotOnWeb`: a page cannot blur what is behind the browser window.
- **R9, R10.** Pointer events (one code path for mouse, touch and pen); native elements keep their roles; custom ones set `role`, `aria-*` and `tabindex` in the view.
- **DataGrid (Q4 decided).** In-memory `Rows` stay a client table (class A use); a table bound to an IndexedFile or SQL source is class C and **paged on the server**: the view asks for a row window and the backend answers that window. The request shape is WASM-005's frame and the server's paging is WASM-005/WASM-023's; this spec's part is a DataGrid view that renders a window and asks for the next one, never the whole table.

### 1.3 Layout (R8) — kept

`layout_css.rs` maps the four layout modes and the breakpoints to CSS; the Rust solver does not run in the browser (Q1 decided).

### 1.4 Form layers on the web (R13–R24) — **new**

The model already holds layers (`crates/cobolt-forms/src/model.rs`: `Form::layers`, `Layer { name, backdrop }`, `LAYER_PROPS`, `layer_writable`, `layer_refusal`, `Layer::runtime_props` with `Visible` false, `Layer::with_live`, `Form::layer_of`, `Form::layer_rank`). The player reuses them; nothing about layers is re-invented.

- **The manifest (A9, R22).** `cobolt-wasm-abi`'s `FormEntry` gains `layers: Vec<LayerEntry { id, name, index }>` with `#[serde(default)]`, so a manifest without layers reads as before (R24). Ids follow the controls: with `n` controls the first layer is `3 + n`. The generator (`cobolt-wasm`, WASM-010 R19) emits them and accepts `SET`/`GET` on a layer by name; a write to `Name`, or a property not in `LAYER_PROPS`, is a **build error** carrying `layer_refusal`'s text (R22, spec 091 R37).
- **The tree.** `FormTree::build` records each control's layer rank from `Form::layer_of` + `Form::layer_rank` (a control inside a container takes its outermost container's layer; an unknown layer name stays in the base, as the desktop does), and a layer node per layer carrying `Layer::runtime_props()` — so every layer starts hidden in every instance (R15, A9).
- **The DOM (R14, R17, R19).** A form instance becomes:
  - a **form root** with `position: relative; overflow: hidden` and the form's `border-radius` (moved from `.pr-form`'s `CornerRadius > 0` branch in `view/style.rs` to the root, and applied always, so the rectangle clips even with radius 0); it clips the base and every layer, for every control type (R17, including the seven that cannot clip themselves on the desktop — the difference WASM-015 R17's guard allows);
  - the **base** (`.pr-form`, as today: CSS layout of R8, its own scrolling);
  - one **layer element** per layer, a sibling after the base, `position: absolute; inset: 0; isolation: isolate; z-index: k` (k = rank), so every control of a higher layer paints above every lower one whatever its `ZOrder` (R14). Inside a layer, controls are absolutely positioned at `X`, `Y`, `Width`, `Height`; no layout, `Dock` or `Anchor` (R19). `ZOrder` orders siblings only (the existing `view/style.rs` sort by `(depth, zorder, index)` becomes per layer and parent).
- **Backdrop (R18).** A layer's background is its own first child (`.pr-layer-bg`, `inset: 0`), painted from `Layer::with_live` (colour, gradient, image, mode); `Transparency` is the **alpha of the background colour/gradient stops**, never `opacity` on the layer element.
- **Visibility (R15).** A hidden layer: `display: none` plus the `inert` attribute. `SET L::Visible TO TRUE` arrives as an A9 `SET` and the next patch shows it (R22).
- **Hit testing (R16, Q7 decided).** The layer element and every area that paints nothing are `pointer-events: none`; a control's painted part is `pointer-events: auto` (for a Label without a background, the text span, not the box). The backdrop element is hit-testable **only when the layer is opaque by its description** — `Transparency` 0 and a colour, or both gradient stops, at full alpha; an image never counts. Its pointer handlers discard the event (no dispatch to the module, `stopPropagation`), so nothing beneath receives it; a transparent backdrop is `pointer-events: none` and the event reaches the layers below. The rule is the desktop's `render.rs::layer_is_opaque`, which this plan **moves to the model** as `Layer::is_opaque()` with a hex-alpha parser that needs no egui, and both hosts call it (one rule, Q7).
- **Keyboard order (R20).** DOM order is the focus order: the base's controls sorted by `TabOrder`, then each layer upward, each sorted by its own `TabOrder`; every focusable element `tabindex="0"` (a disabled or hidden one `-1`); **no positive `tabindex`** (C5 replaces `with_tab_order`). Because DOM order now carries the tab order, the visual stacking inside a parent comes from `z-index` per `ZOrder`, not from document position.
  - *Working assumption (not stated by the spec):* the desktop also shields **focus** from controls beneath the highest shown opaque layer (`render.rs::opaque_shield_rank`); the player matches it by making the base and lower layers `inert` while such a layer is shown. Parity with the desktop is the reason; the operator may rule otherwise at `/clarify`.
- **Non-visuals (R21).** Non-visual controls produce no element and belong to no layer (the view already skips them, `view/style.rs:386`).
- **Snackbar (Q8 decided).** The toast stack is a child of the form root after the last layer, with a `z-index` above every layer; the control stays non-visual.
- **Containers (R23).** A layered form in an SPA ContentPane (WASM-011) or a standalone window is the same form root, sized to the area the form occupies, so its layers cover that area.
- **No layers, no change (R24).** A form with an empty `layers` produces the same manifest (the field is skipped when empty) and the same DOM apart from the form-root wrapper; the player's existing view tests are the guard, updated once for the wrapper.

### 1.5 The IDE half (R5, R6, R12) — **revised**

- **Toolbox (R5).** `crates/cobolt-ide/src/panels/toolbox.rs::ToolboxPanel::show` filters its `TOOLS` rows by the project kind: on Web and Mobile a class D type is not listed; the toolbox's existing test that every `ControlType::ALL` has a `TOOLS` row stays. Plug-in (`Custom`) controls are class D on the web until WASM-032.
- **Check (R5, AC4).** The web build gate already reports refusals as lines `file:line: construct — alternative` (`crates/cobolt-project-tools/src/web/mod.rs::WebBuildError::Refused`). A new check over every `.wcfrm`/`.mcfrm` reports each class D control, and each use of a class D operation of a multi-class control in a handler, in that format, with the alternative from the table's row; the line is the control's line in the form file. It runs in Check as well as Build, so the developer sees it before building.
- **Properties panel (R6, Q3 decided).** In `panels/properties.rs::show_selection`, when the form is a web or mobile form, a row whose property is `NotOnWeb` in the shared fidelity table is **not drawn**; its value is untouched in the model, so the file keeps it (AC5). An `Approximate` row stays editable and its tooltip gains the approximation note. The toolbox and the event-binding editor are unchanged.
- **System KB (R12, AC8).** The compiler's control documentation (`crates/cobolt-compiler/src/lib.rs`: `control_structure`, `control_purpose`, `controls_reference_doc`, and the per-control usage notes) gains each control's web class and its per-operation exceptions, generated from the shared table, plus any web-only member (none yet). `assets/knowledge/chunked.data` is regenerated in the same change and `grace_host.rs::prebuilt_chunked_kb_matches_the_published_documentation` is the gate. The agent kit's control list (`crates/cobolt-ide/src/agent.rs`) reads the same table so an agent never proposes a class D control for a web form.
- **i18n.** New `Tr` fields ×6 languages in `crates/cobolt-ide/src/i18n.rs`: the toolbox note that unavailable controls are hidden for this kind, the "approximate on the web" tooltip prefix, and the Check message frame (the per-control alternatives come from the table and are English in the tools, translated in the IDE as the build gate's messages already are).

### 1.6 Fonts (Q5 decided) — **new**

The desktop draws `Arial`/`Helvetica`/default with egui's built-in proportional face and loads any other family from the system (`crates/cobolt-forms/src/fonts.rs`: `is_builtin`, `load_font_bytes`). The web Build (`cobolt-web-bundle`) bundles, as `@font-face` files under the bundle's assets: the face the desktop uses for the built-in family, and every family a form names that the project supplies under `Assets/`. The player's `--pr-font` then names the bundled family first and the system stack after it. When the font files exceed **50 % of the application's size** (the bundle's size by part already exists), the Build adds a warning to `warnings` saying so and suggesting system fonts only. A family that only the developer's operating system provides is **not** copied (see §5, licences) — it falls back to the system stack and the Build says which family did.

### 1.7 Touch hit area (Q9 decided) — **new**

Under `@media (pointer: coarse)`, every interactive control's hit area is 15 % larger than its painted part (7.5 % each side, centred): a transparent `::after` on the control's root element with negative inset, `pointer-events: auto`. Replaced elements (`<input>`, `<img>`, `<select>`, `<progress>`, `<textarea>`) have no pseudo-elements, so their view wraps them in a `<span>` that carries the extension. Where two enlarged areas overlap, the browser's stacking decides (the topmost wins), as for any hit. Non-interactive controls (Label without a click handler, Line, Shape) are not enlarged.

## 2. Affected crates / files

- `crates/cobolt-forms/src/web.rs` (new) — the class table, the per-operation exceptions, the by-name series kinds, the property-fidelity rows (C1, C3).
- `crates/cobolt-forms/src/model.rs` — `Layer::is_opaque` (moved from `render.rs::layer_is_opaque`, which calls it); a hex-alpha helper without egui.
- `crates/cobolt-forms/src/render.rs` — `layer_is_opaque` becomes a call to the model's.
- `crates/cobolt-wasm-abi/src/manifest.rs` — `FormEntry::layers`, `LayerEntry` (A9).
- `crates/cobolt-wasm/` — emits layer ids and entries; `SET`/`GET` on a layer by name; the build error for `Name` or an unknown layer property (WASM-010 R19 owns the lowering; this spec owns the contract it serves).
- `crates/cobolt-web-player/src/kinds.rs` — reads the shared table; `is_non_visual` delegates to the model (C6).
- `crates/cobolt-web-player/src/css.rs` — keyed by the shared fidelity rows.
- `crates/cobolt-web-player/src/tree.rs` — layer rank per control, layer nodes, layer `SET`/`GET`.
- `crates/cobolt-web-player/src/view/{style,controls,mod}.rs`, `base_css.rs` — form root, layer elements, backdrop, `inert`, pointer rules, per-layer `ZOrder`, DOM-order tab order (C5), Snackbar above layers, touch extension, bundled font name.
- `crates/cobolt-web-player/src/view/data.rs` — DataGrid row window for a remote binding.
- `crates/cobolt-web-bundle/src/lib.rs` — font files and the 50 % warning.
- `crates/cobolt-project-tools/src/web/mod.rs` — the class D check lines.
- `crates/cobolt-ide/src/panels/toolbox.rs`, `panels/properties.rs`, `agent.rs`, `i18n.rs` — R5, R6, agent kit, `Tr` ×6.
- `crates/cobolt-compiler/src/lib.rs` + `assets/knowledge/chunked.data` — R12.
- **Not** `README.md`, **not** `docs/` (2.x.x line). What 2.x.x says about itself stays in `specs/`.

## 3. Data / model changes

- **Form files:** none. A web or mobile form is the `.cfrm` schema under `.wcfrm`/`.mcfrm` (WASM-001). Layers are already in the schema (spec 091). A property hidden by R6 keeps its stored value; nothing is removed on load or save (AC5; user content is never dropped).
- **Manifest (`prmanifest`):** `forms[i].layers` added per A9, optional and skipped when empty, so existing manifests and bundles are unchanged (R24).
- **`cobolt-forms` API:** a new `web` module (additive); `Layer::is_opaque` (additive). No public type changes shape.
- **Player:** `tabindex` values change from `TabOrder` numbers to `0` (C5) — a behaviour change confined to the player, held by tests.

## 4. Key decisions & alternatives

- **Decision: the class and fidelity tables live in `cobolt-forms`** (C1). Why: R2 names one place read by the toolbox, Check, KB and agents; `cobolt-forms` is the one crate the IDE, the compiler, the project tools and the player all depend on, and its model part already builds for `wasm32`. Rejected: the IDE and compiler depending on `cobolt-web-player` (pulls Leptos and `web-sys` into native crates); two tables kept equal by a test (two sources is what R2 forbids).
- **Decision: layers as sibling stacking contexts inside a clipping form root**, the base keeping its own scroll. Why: one element clips everything (R17), each layer is a stacking context (R14), and a scrolled base does not drag the layers with it (R19: layers are positioned at their designed rectangle over the form). Rejected: layers inside `.pr-form` (they would scroll with the base); one `z-index` per control computed across layers (fragile, and no `isolation` for a layer's own content).
- **Decision: hit testing by the browser, with `pointer-events` set from the model's own rule** (`Layer::is_opaque`). Why: R16 asks for the browser's hit testing and Q7 for the description rule; sharing the predicate keeps desktop and web equal. Rejected: a JavaScript or Rust hit-test over geometry (a second implementation of the browser's).
- **Decision: DOM order carries the tab order; `tabindex` is `0` or `-1` only** (C5). Why: R20; positive values number across the whole document and break the per-layer order, and with several form instances on one page (SPA) across instances too. Cost: the view now sorts siblings by `TabOrder` and draws stacking by `z-index`.
- **Decision: hide `NotOnWeb` rows, mark `Approximate` rows** (Q3 operator ruling). Rejected: hiding approximate rows too (the spec keeps them editable).
- **Decision: fonts — bundle what can lawfully be shipped** (Q5 ruling), system fonts only as fallback for a family the project does not supply.
- **Decision (kept): the view is data (`VNode`), the page is a patcher**; keystrokes refused by writing the stored text back; page-only state applied when the event arrives; tiles `crossorigin="anonymous"`; PDF text and size only (rationale in the 2026-10-20 plan, unchanged).
- **Q2 — open (not answered by the operator).** Working assumption, matching the spec's proposed table and today's `kinds.rs` rows: **Animator B** (the browser plays the formats; the richer API is commands), **Timer A** (no element; a player timer), **DateTimePicker A** (native `input type=date|time|datetime-local`; differences from the desktop picker listed per R4's rule), **TreeView A** (`ul role=tree`), **DataGrid C** primary with A for in-memory `Rows`. Each is one row of the shared table, so a different ruling is a one-row change plus its tests.
- **D20 in `WASM-INDEX.md` is still "Proposed"** (the HTML realisation of layers to confirm at `/clarify`). This plan follows R13–R24 as the spec states them and decides nothing beyond them except the marked working assumption on focus shielding.

## 5. Risks & mitigations

- **Font licences.** Bundling a family the developer's OS provides (Arial, Segoe UI, Helvetica) is redistribution the font's licence usually forbids. → Bundle only the built-in face and project-supplied `Assets/` fonts; name each skipped family in the Build's output. *Open sub-question for the operator:* whether the Build should also refuse, rather than warn, a form that names a family it cannot ship.
- **Touch extension overlapping neighbours** steals taps between close controls. → Topmost wins (browser stacking); the AC for touch is measured on a dense reference form in a narrow viewport; WASM-021 owns the full touch model.
- **Re-marking fidelity after WASM-007's CSS** could unhide or hide properties forms already rely on. → Values are never removed (AC5 test); the row change is reviewed with WASM-007's CSS evidence.
- **Behaviour change of tab order** (C5) for forms that set `TabOrder`. → The new order is the same sequence the desktop walks; a test compares the desktop's order (the model's `TabOrder` sort) with the DOM order for a reference form.
- **Layers and the ABI** need the generator (WASM-010) to emit A9 before AC10 can run end to end. → The player side is tested with a hand-built manifest (as WASM-016's fixture module does); the end-to-end run is recorded when the generator lands.
- **Desktop and browser differ** (AC3, AC6) — still no shared harness. → §6's comparison harness is new work, not optional.
- **Accessibility** — no automated audit was run yet (AC7). → §6.
- **Old browsers** without the Custom Highlight API (Find) or `inert` (layers). → Find degrades to count and scroll; for `inert`, `display: none` already removes a hidden layer from focus and hit testing, so R15 holds without it.

## 6. Test strategy

Existing (kept, re-run): `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib` — 220 tests including the table-versus-model test (AC1), a view test per control, the interaction tests (keys, pointer, Find, Picture, FileDropZone, map pan/zoom, knob), `methods` and `viewer`; and the Chrome 152 runs of 2026-10-20 (gallery bundle, 127.0.0.1 only): TextBox Picture, Find (`1 / 14`, wrap, match case 12, no `<mark>`), Hyperlink (`<a … rel="noopener noreferrer">`, no `ping`, `javascript:` refused, `mailto:` accepted), Map pan and zoom (6/6 tiles), Knob 35 → 50, FileDropZone drop, PDF `Page 1 of 1 (612 x 792 pt)`, Mermaid, Canvas2D. Measured parts: Mermaid 1,906,933 bytes, PDF 219,273, Canvas 469,419.

New, native (`cargo test`), each printing what it compared:

- `cobolt-forms` — the web table covers `ControlType::ALL` exactly once and every series name once (AC1, moved); every multi-class exception names an operation the control has (methods in `methods.rs`); the fidelity rows equal the player's CSS rows; `Layer::is_opaque` on colour, gradient, transparency and image cases, and `render.rs` gives the same answers.
- `cobolt-web-player` — `is_non_visual` equals the model predicate over `ALL` (R21); a layered fixture form: DOM order is base then layers, `z-index` per rank, a layer control with `ZOrder` 0 above a base control with `ZOrder` 209 (AC9 natively); a hidden layer has `display: none` and `inert`, and `Visible TRUE` removes both on the next patch (AC10 natively); an opaque backdrop is hit-testable and its handler dispatches nothing, a transparent one is `pointer-events: none` (AC11 natively); tab order from `TabOrder` per layer with only `0`/`-1` values (AC12 natively); the form root clips with `overflow: hidden` and the radius (AC13 natively); a form without layers produces the same manifest bytes and the same view as before (R24); the Snackbar stack is above the top layer; the touch extension appears only under the coarse-pointer rule; the DataGrid of a remote binding asks for a window and renders it.
- `cobolt-wasm-abi` / `cobolt-wasm` — a manifest without `layers` round-trips unchanged; with layers the ids are `3 + n …`; `SET L::Name` and `SET L::Bogus` fail the build with `layer_refusal`'s text.
- `cobolt-web-bundle` — fonts copied for the built-in family and an `Assets/` family; a system-only family skipped and named; the warning appears above 50 % and not below.
- `cobolt-project-tools` — a `.wcfrm` with a class D control and a handler calling a Viewer's `Print` fail Check with `file:line: … — alternative` lines (AC4).
- `cobolt-ide` (`--bin cobolt-ide`) — the toolbox on a Web project lists no class D type (AC4); the Properties panel of a web form omits `NotOnWeb` rows and marks `Approximate` ones; AC5: a desktop form saved and reloaded as a web form keeps every property value, hidden ones included; `prebuilt_chunked_kb_matches_the_published_documentation` green after the regenerated store (AC8); `Tr` fields present in six languages.

New, in the browser (Chrome 152, 127.0.0.1, the gallery bundle plus a layered reference form) — results recorded in tasks.md with numbers:

- AC9–AC13 end to end once the generator emits A9: stacking, start-hidden then shown by `SET … Visible TO TRUE`, the opaque/transparent click, Tab through base then each visible layer, a DataGrid at a rounded corner inside the arc (a pixel read at the corner).
- **AC2**: for each class A control, set each property to a distinct value and read back from the DOM (the browser counterpart of `tests/controls/`' `assert_control`).
- **AC3 / AC6 — the desktop comparison harness (new)**: the same fixed data rendered by the desktop engine (`cobolt-forms` `render` feature, headless) and the player; geometry compared per control, colours sampled for class B; event logs from a scripted interaction (click, change, focus, key) compared in order. Documented differences are the only accepted ones; each found difference is listed per control (R4).
- **AC7**: a keyboard-only walk over the reference form, and an automated accessibility audit (axe-core run from the headless browser by the test tool outside the repository — the PRIME DIRECTIVE keeps the repository Rust-only); no critical issue.
- Fonts: the bundled family is the one the computed style uses (no fallback), and the bundle's size report shows the font share.

## 7. Steering compliance

- [ ] i18n: the new IDE strings (toolbox note, approximate tooltip, Check frame) as `Tr` fields in six languages — planned, not done.
- [x] Generated-code banner and regenerate-on-action: unchanged for desktop; the web output is downstream of the generated COBOL.
- [ ] System KB tables (R12) and the regenerated `chunked.data` with the freshness test — planned, not done.
- [x] Interpreter/binary parity: no interpreter change. The one shared rule that moves (`layer_is_opaque` → `Layer::is_opaque`) keeps the desktop's answers, held by a test in both hosts' crate.
- [x] PRIME DIRECTIVE: no non-Rust source added to the repository; browser tooling stays outside the tree.
- [x] User code is sacred: hidden properties keep their values; nothing in a form file is dropped.
- [x] COBOL identifiers English; no "cobolt" in user-facing text; no invented property or method (Find's members are spec 058's own; layer properties are `LAYER_PROPS`).
- [x] `README.md` and `docs/` untouched (2.x.x line).
- [ ] Fix vs feature: **feature** on `features-2x`; version bump of `z` and the CHANGELOG entry with the implementing commit.
