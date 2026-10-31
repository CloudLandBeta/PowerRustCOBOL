<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# Tasks — Control reuse on the web (WASM-004)

- **Status:** in progress — the player half (T1–T10) is done and tested; T12 and T13 (the shared table, its per-operation exceptions and fidelity rows) done at 2.0.46; **stopped at T14**, which now needs an operator decision (re-checked 2026-10-31 at 2.0.113: no WASM-007 task draws the border rows; see T14). Not started: layers in the view, the IDE half, fonts, touch, the harnesses. Note for T16/T17: `FormEntry::layers`/`LayerEntry` (`crates/cobolt-wasm-abi/src/manifest.rs`) and the player tree's `LayerState` (`crates/cobolt-web-player/src/tree.rs`) already landed with WASM-016 part 1 (2.0.41); check them against those tasks' verifications before writing anything
- **Plan:** ./plan.md   **Date:** 2026-10-22 (first written 2026-10-20; revised to match the plan at 2.0.29)

Ordered, small, independently verifiable tasks. Each names the files it touches, the requirements it satisfies and how it is verified. Native tests print what they compared. Browser runs use Chrome 152 on 127.0.0.1 only, and their numbers are written here when they are run.

Commands used below:
- forms: `cargo test -p cobolt-forms --features render <filter>` (the `render` feature is needed for the desktop half of a shared rule)
- player: `cargo test --manifest-path crates/cobolt-web-player/Cargo.toml --lib <filter>` (the player is outside the workspace, root `Cargo.toml` `exclude`)
- IDE: `cargo test -p cobolt-ide --bin cobolt-ide <filter>`

Working assumptions (open questions; the operator may rule otherwise, and each is one row or one rule to change):
- **Q2 — not answered.** Animator **B**, Timer **A**, DateTimePicker **A** (native `input type=date|time|datetime-local`, differences listed per R4), TreeView **A** (`ul role=tree`), DataGrid **C** primary with **A** for in-memory `Rows`. These are today's rows in `crates/cobolt-web-player/src/kinds.rs::TABLE`.
- **Focus shielding (plan §1.4, not stated by the spec).** While an opaque layer is shown, the base and the lower layers are `inert`, matching the desktop's `crates/cobolt-forms/src/render.rs::opaque_shield_rank`.
- **ToolBar actions not named by the spec.** The spec names as class D the actions that *start a program, open a terminal or print through the OS*: `ToolbarAction::RunApp` (`run-app`), `OpenTerminal` (`open-terminal`) and `Print` (`print`) (`crates/cobolt-forms/src/toolbar.rs::ToolbarAction`). `Share` is taken as D, as the spec makes the Viewer's share D. `Screenshot`, `Copy`, `Cut` and `Paste` are not named; they are taken as A (the browser's clipboard and a canvas capture of the page), to confirm.
- **Font licences (plan §5).** A family only the developer's OS provides is skipped with a Build warning, not refused.
- **D20 (`WASM-INDEX.md`, "Proposed").** The layer tasks follow R13–R24 as the spec states them.

## Done (kept from 2026-10-20, with their evidence)

- [x] **T1 — The class table** (R2, AC1): `crates/cobolt-web-player/src/kinds.rs`; `the_table_and_the_model_agree_on_every_control` fails when `ControlType::ALL` and the table disagree. *(T12 moves the table into `cobolt-forms`; the test moves with it.)*
- [x] **T2 — Class A controls as native elements** (R3, R9, R10): `src/view/controls.rs`, `data.rs`, `shell.rs`, `auth.rs`; roles, names and `tabindex` asserted natively; TabOrder -> `tabindex` (stop types only). *(T20 replaces the numeric `tabindex` with DOM order, R20.)*
- [x] **T3 — Class B controls** (R4): charts (`charts.rs`), Maps (`maps.rs`), Knob / Gauge / Switch / Slider, Shape / Line, Animator, Viewer, Canvas2D.
- [x] **T4 — Layout by CSS** (R8): `src/layout_css.rs` (four modes, breakpoints); the Rust solver does not run in the browser.
- [x] **T5 — TextBox Picture** (R3, spec 004's numeric-edit): `view/controls.rs::text_box`, `view/interact.rs::text_box`, `tree.rs::picture_of`; decimal comma and currency from `SPECIAL-NAMES`. Chrome: edited form at rest, raw while focused, invalid keys refused, sign normalised on blur.
- [x] **T6 — Viewer: Markdown, HTML, text, Mermaid, PDF, Find** (R4): the parts (WASM-016); Find with Custom Highlight API; PDF page size and text. Chrome: Find `1 / 14`, wrap, match case, no `<mark>`; PDF `Page 1 of 1 (612 x 792 pt)` plus its text.
- [x] **T7 — Maps: pan, zoom, tiles** (R4): drag, wheel, buttons, keys; 6 of 6 tiles on an isolated page. Chrome: a drag moved the tiles by the drag's distance; the zoom buttons changed the zoom level.
- [x] **T8 — Knob drag and FileDropZone** (R4, R9): Chrome: knob 35 -> 50 by a drag to the top; a dropped file listed by the zone.
- [x] **T9 — Canvas2D through the Canvas part** (R4; WASM-018 R7): `methods.rs::canvas_method`, `dom/canvas.rs`, `parts/canvas`. Chrome: fills, a line and `Present` put the expected pixels (red 255,0,0,255; blue 32,96,192,255; untouched 0,0,0,0) on a 240x120 canvas, the part fetched only after the first call.
- [x] **T10 — DataGrid sort and filter on Rayon** (WASM-017 R19a): threaded variant; result equals a stable JS sort on 300,000 rows (see WASM-016).

## Open, carried over

- [ ] **T11 — Canvas2D read-backs and Canvas3D** (R4; owned by WASM-018 T11, tracked here because R4 asks for the same methods)
  - Today `GetPixel`, `MeasureText`, `IsPointInPath`, `ToImage`, `Capture` are refused with the reason (no synchronous answer while the Canvas part loads asynchronously); Canvas3D has no part.
  - Verify: as WASM-018 T11 records it; this spec's AC3 run (T33) lists the refusal as a documented difference until then.

## One definition (R1, R2, R7, R21)

- [x] **T12 — The class table moves into `cobolt-forms`** (R2, AC1; plan C1)
  - *Done 2.0.46.* The move itself had already happened in WASM-001 T14: the module is `crates/cobolt-forms/src/web_class.rs` (not `web.rs`) and the enum is `Class` (not `WebClass`); kept under those names because the toolbox and the player already read them. Added there: `Operation { Method, ToolbarVerb, Property }`, `Exception { operation, class, alternative }`, `exceptions_of`, `unavailable_operation`, `alternative_of`, `PlayerKind::series_name` (`Hyperlink`; `WebLink` still recognised and still the player's own spelling until WASM-006). Evidence: `cargo test -p cobolt-forms --features render --lib web_class` 6 passed; class sizes printed `A: 26 B: 12 C: 24 D: 0 E: 0`; 15 exceptions (Maps 6, Viewer 4, ToolBar 4, DataGrid 1), each checked against `is_known_method`, `ToolbarAction::VERBS` or the DataGrid's seeded properties; `cargo build -p cobolt-forms --no-default-features` and `--target wasm32-unknown-unknown --no-default-features` both finished.
  - Files: `crates/cobolt-forms/src/web.rs` (new, no egui), `crates/cobolt-forms/src/lib.rs` (`pub mod web`).
  - Do: `WebClass { A, B, C, D, E }`; one row per `ControlType::ALL` entry (class, what it becomes, note) copied from `kinds.rs::TABLE` with the Q2 working assumption; the series kinds the model holds as `Custom` by name (`Hyperlink` — WASM-006's name; `WebLink` accepted as its old spelling until WASM-006 renames the player — `Authentication`, `Canvas2D`, `Canvas3D`, `ImageBuffer`, `Plot`, `Scene3D`, `Physics2D`, `Physics3D`); `Custom` plug-ins class D. **Per-operation exceptions** as data, spelled as `crates/cobolt-ast/src/methods.rs::is_known_method` spells them: Maps `Geocode`, `ReverseGeocode`, `Directions`, `DistanceMatrix`, `PlacesSearch`, `TraceRoad` → C; Viewer `LoadBytes` is not an exception (no disk), `SaveAs`, `SaveAsPdf` → C, `Print`, `Share` → D; ToolBar actions (by `ToolbarAction::verb`) `run-app`, `open-terminal`, `print`, `share` → D (working assumption above); DataGrid in-memory `Rows` → A; each row carries its English alternative for Check. `pub fn class_of(&ControlType) -> WebClass`, `pub fn exceptions_of(&ControlType)`.
  - Verify: forms `web::` — the table covers `ControlType::ALL` exactly once and each series name once (AC1, moved from the player); every exception names a method `is_known_method` accepts or a `ToolbarAction` verb (no invented name); class sizes printed and asserted (B 12, C 24 over `ALL`). `cargo build -p cobolt-forms --no-default-features` and `cargo build -p cobolt-forms --target wasm32-unknown-unknown --no-default-features` succeed.

- [x] **T13 — The fidelity rows move with the table; the player reads both** (R1, R2, R6, R7, R21; plan C1, C6)
  - *Done 2.0.46.* `cobolt_forms::web_class::{Fidelity, PropertyFidelity, FIDELITY, fidelity_of}` (46 rows: 35 exact, 7 approximate, 4 not on the web); the player's `css.rs::TABLE` now holds only `Mapping { property, css }` (+ `mapping_of`) and re-exports the shared `fidelity_of`; `kinds.rs::is_non_visual` delegates to `ControlType::is_non_visual()` for model types. Evidence, player `--lib --no-fail-fast`: **230 passed, 0 failed**; `the_non_visual_set_is_the_models_…` (24 of 62 non-visual, equal); `the_css_rows_and_the_shared_fidelity_rows_name_the_same_properties` (46 in both); `every_property_the_view_reads_is_a_model_or_a_layer_property` — 141 names read by `tree.get(_, "…")` across `src/view/*.rs`, 141 known (`property_names_for`, `runtime_property_names_for`, `LAYER_PROPS`; allowed besides: names with a leading `_`, the desktop's internal state such as `__ChartData`, and TreeView `SelectedNode`, which the desktop host writes but `runtime_property_names_for` does not list). The player still builds for `wasm32-unknown-unknown`.
  - Files: `crates/cobolt-forms/src/web.rs` (property → `Exact | Approximate | NotOnWeb`, note — the rows of `crates/cobolt-web-player/src/css.rs::TABLE`/`Fidelity`), `crates/cobolt-web-player/src/kinds.rs` (thin reader: `class_of`, `PlayerKind` keep their shape for `host.rs`, `view/style.rs`), `crates/cobolt-web-player/src/css.rs` (CSS strings and declaration functions stay, keyed by the shared rows).
  - Do: `kinds.rs::is_non_visual` calls `ControlType::is_non_visual()` (`model.rs:4152`) for model types and keeps `!PlayerKind::visual()` for the by-name kinds.
  - Verify: player — `kinds::` the non-visual set equals the model predicate over `ControlType::ALL` and every E kind is non-visual (R21); `css::` a property in the shared fidelity rows and not in the CSS rows (or the reverse) fails; a guard test that every name the view reads with `cx.tree.get(i, "…")` is a model property or in `model.rs::LAYER_PROPS` (R1, R7: no web-only member by accident). All 220 existing player tests stay green.

- **Decided (operator, 2026-10-31):** (1) the border look is **drawn in CSS** — T13b below adds it, and T14 re-marks the rows from what T13b draws; (2) T14 does **not** gate the tasks after it: T15–T21 do not depend on it and run without it, so WASM-007 T15 (which waits for T18) is no longer held behind T14.

- [ ] **T13b — The border look drawn in CSS** (R4, R6, Q6; operator 2026-10-31)
  - Files: `crates/cobolt-web-player/src/css.rs` (`TABLE`), `crates/cobolt-web-player/src/view/style.rs`, `crates/cobolt-forms/src/web_class.rs` (`FIDELITY`), the player's tests; the prebuilt player in `assets/web-player/` rebuilt.
  - Do: the player reads and draws `BorderGradientEnabled` with `BorderGradientStartColor` / end colour / direction (a gradient border: `border-image` or a background-clip layer, rounded corners kept), `BorderBlur` (`backdrop-filter`/`filter` on the border band), `BorderTransparency` (alpha on the border colour), the Snackbar's corner rows, and `PerspectiveStrength` (CSS `perspective` / `transform`), matching the desktop renderer (`cobolt-forms` `paint.rs`) as closely as CSS allows.
  - Verify: player tests show each property reaching the computed style of a control; a Chrome capture of a form using all four next to the desktop `render_form` picture, numbers written here.

- [ ] **T14 — Fidelity rows re-marked against Q6** (R4, R6; plan C3; depends on WASM-007's CSS)
  - Files: `crates/cobolt-forms/src/web.rs` (rows only).
  - Do: for each row now `NotOnWeb` or `Approximate` (`BorderGradientEnabled`, `BorderBlur`, `BorderTransparency`, the shadow rows, …), set it from what WASM-007's CSS actually draws (Q6 decided: reproduce the look in CSS); `OsBlur` stays `NotOnWeb`. Each changed row cites WASM-007's evidence in its note.
  - Verify: forms `web::` — a printed list of `NotOnWeb` and `Approximate` rows, reviewed against WASM-007's tasks.md evidence; T27's AC5 test still green (values never removed).
  - **Blocked (2026-10-23, /implement at 2.0.46): WASM-007's look CSS does not exist yet.** Audit of each non-exact row against what `crates/cobolt-web-player/src/css.rs` draws today: `BorderStyle` (two-tone border, `border`), `ShadowEnabled` (the dual `box-shadow`), `ShadowBlurStrength` (blur radius ×2, inset when negative), `RotateX`/`RotateY`/`PerspectiveDistance` (`transform: perspective(400 + 12·d px)`, in `appearance`) all match their `Approximate` mark; `BorderGradientEnabled`, `BorderBlur`, `BorderTransparency` are read by nothing, which matches `NotOnWeb`; `OsBlur` stays `NotOnWeb` by the plan; **`PerspectiveStrength` is marked `Approximate`, but nothing reads it**. Q6 asks for the look in CSS, and no open WASM-007 task (T7–T30) draws a border gradient, a border blur or a border transparency. Re-marking from today's CSS would hide (R6) properties the operator wants reproduced. Re-marking from CSS not yet written would be guesswork. Resume once WASM-007 has added that CSS.
  - **Re-checked 2026-10-30 at 2.0.102, still blocked.** `crates/cobolt-web-player/src/css.rs` is unchanged since 2.0.65: `BorderGradientEnabled` (`border-image`), `BorderBlur` and `BorderTransparency` (`—`) exist only as `TABLE` mappings and no declaration function reads them; `PerspectiveStrength` is still mapped but unread (the `:498` perspective block reads `PerspectiveDistance` only). WASM-007 T7 (probe) has run; T8–T30, including T12 (the chart `BorderGradient*`, `BorderBlur`, `BorderTransparency` rows), are all open. T15 onward does not depend on T14, but this run stops here as instructed.
  - **Re-checked 2026-10-31 at 2.0.113, blocked on an operator decision, not on WASM-007 any more.** WASM-007 T8–T14 have landed (`1bbd46fc`). Its T12 added the rows `BorderGradientStartColor`, `BorderGradientEndColor`, `BorderGradientDirection` and the Snackbar's four `CornerRadius*` rows as `NotOnWeb`, because nothing in the player reads them (`crates/cobolt-forms/src/web_class.rs::FIDELITY`, `:190`–`:199`). `crates/cobolt-web-player/src/css.rs::TABLE` still only maps `BorderGradientEnabled` (`border-image`), `BorderBlur` and `BorderTransparency` (`—`), and no file under `src/view/` reads any of them. `PerspectiveStrength` (`:189`, `Approximate`) is still read by nothing. The open WASM-007 tasks (T15–T30: layer guard, stylesheet files, Build gate, IDE rows, modal, Preview, KB, browser runs, finalize) include no task that draws a border gradient, border blur, border transparency or `PerspectiveStrength`. So no CSS will ever exist for this task to re-mark from. Q6 ("reproduce the look in CSS") then conflicts with this task's "rows only" scope. **The operator chooses one:** (a) add a task, here or in WASM-007, that draws these properties in `css.rs`, then re-mark them as `Approximate`/`Exact`; or (b) accept them as `NotOnWeb`, so this task re-marks only `PerspectiveStrength` (→ `NotOnWeb`, or a task to read it). Also note: WASM-007 T15 waits for this spec's T18, and this run stops before T18.

## Form layers in the player (R13–R24)

- [ ] **T15 — `Layer::is_opaque` in the model, shared with the desktop** (R16, Q7; plan C9)
  - Files: `crates/cobolt-forms/src/model.rs` (`Layer::is_opaque` and a hex-alpha helper without egui), `crates/cobolt-forms/src/render.rs` (`layer_is_opaque` at `:435` calls it).
  - Verify: forms `layer_is_opaque` — opaque colour; colour with alpha < FF; `Transparency` > 0; both gradient stops opaque; one stop translucent; an image (never opaque); the same cases through `render.rs` give the same answers.

- [ ] **T16 — Manifest amendment A9: `FormEntry::layers`** (R22, R24)
  - Files: `crates/cobolt-wasm-abi/src/manifest.rs` (`LayerEntry { id, name, index }`, `FormEntry::layers` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]`).
  - Do: ids `3 + n_controls + k`. The generator side (emitting the entries, `SET`/`GET` on a layer, the build error with `layer_refusal`'s text for `Name` or a property outside `LAYER_PROPS`) is WASM-010 T12; this task only fixes the contract.
  - Verify: `cargo test -p cobolt-wasm-abi manifest` — a manifest without `layers` reads and writes byte-identical (R24); with two layers the ids are `3 + n` and `4 + n`.

- [ ] **T17 — The player's tree knows layers** (R13, R15, R22)
  - Files: `crates/cobolt-web-player/src/tree.rs` (`FormTree::build`), `crates/cobolt-web-player/src/host.rs` (layer `SET`/`GET` by id).
  - Do: each control's layer rank from `Form::layer_of` + `Form::layer_rank` (outermost container's layer; unknown name stays in the base); one node per layer carrying `Layer::runtime_props()` (`Visible` false); writes allowed by `layer_writable`, refused with `layer_refusal`'s text.
  - Verify: player `tree::` — a layered fixture form (hand-built manifest, as WASM-016's fixture module) gives the expected ranks, every layer starts hidden in every instance, `SET Name` refused with the model's text.

- [ ] **T18 — Form root, layer elements, backdrop, visibility, Snackbar** (R14, R15, R17, R18, R19, R21, R23, Q8; AC9, AC10, AC13 natively)
  - Files: `crates/cobolt-web-player/src/view/{mod,style}.rs`, `base_css.rs`.
  - Do: a form root `position: relative; overflow: hidden` with the form's `border-radius` always (moved from `.pr-form`'s `CornerRadius > 0` branch); the base `.pr-form` unchanged; one layer element per layer after it, `position: absolute; inset: 0; isolation: isolate; z-index: rank`; layer controls absolute at `X`/`Y`/`Width`/`Height`, no layout, `Dock` or `Anchor`; `ZOrder` sort by (layer, parent, zorder, index); backdrop `.pr-layer-bg` from `Layer::with_live`, `Transparency` as the alpha of colour/gradient stops, never `opacity`; hidden layer `display: none` + `inert`; non-visuals produce no element; the Snackbar stack after the last layer with a `z-index` above every layer; the root sized to the area it occupies (ContentPane or window).
  - Verify: player `view::layers_` — a layer control with `ZOrder` 0 has a higher stacking context than a base control with `ZOrder` 209 (AC9); a hidden layer has `display: none` and `inert`, and a `Visible TRUE` set removes both on the next patch (AC10); the root carries `overflow: hidden` and the radius, a DataGrid in a layer is inside it (AC13); the backdrop of a 50 % layer has alpha 0x80 and no `opacity`; the toast stack is above the top layer.

- [ ] **T19 — Hit testing by the browser** (R16, Q7; AC11 natively)
  - Files: `crates/cobolt-web-player/src/view/{style,mod}.rs`, `base_css.rs`, the backdrop's handler in `view/interact.rs`.
  - Do: layer element and empty areas `pointer-events: none`; a control's painted part `auto` (a Label with no background: its text span); the backdrop hit-testable only when `Layer::is_opaque()`, its handler dispatches nothing and stops propagation; the focus-shield working assumption (`inert` on the base and lower layers while an opaque layer is shown).
  - Verify: player `view::layer_hit_` — opaque backdrop `pointer-events: auto` and its handler sends no event to the module; transparent backdrop `none`; Label text span `auto`, box `none`; the shield toggles with the opaque layer's `Visible`.

- [ ] **T20 — Tab order from DOM order, `tabindex` 0 or −1 only** (R10, R20; AC12 natively; plan C5)
  - Files: `crates/cobolt-web-player/src/view/controls.rs` (`with_tab_order` replaced), `view/style.rs`.
  - Do: siblings emitted base first by `TabOrder`, then each layer upward by its own `TabOrder`; focusable `tabindex="0"`, disabled or hidden `-1`; stacking from `z-index` per `ZOrder`, no longer from document order.
  - Verify: player — `the_tab_order_is_the_browsers_focus_order_on_the_element_that_takes_the_focus` (`controls.rs:867`, today expecting `"3"`) rewritten to assert DOM order and `"0"`; a layered form's focus order equals the model's `TabOrder` sort per layer; no `tabindex` value above 0 anywhere in the view of the gallery form.

- [ ] **T21 — No layers, no change** (R24)
  - Files: tests only (`crates/cobolt-web-player/src/view/`, `crates/cobolt-wasm-abi`).
  - Verify: a form without layers yields the same manifest bytes as before and the same view apart from the form-root wrapper; the existing view tests updated once for the wrapper and otherwise unchanged (the diff of expectations is printed).

## Decided behaviours (Q4, Q5, Q9)

- [ ] **T22 — Touch hit area 15 % larger** (R9, Q9)
  - Files: `crates/cobolt-web-player/src/base_css.rs`, `view/controls.rs::root` users.
  - Do: under `@media (pointer: coarse)` a transparent `::after` with 7.5 % negative inset on each interactive control; replaced elements (`input`, `img`, `select`, `progress`, `textarea`) wrapped in a `<span>` carrying it; Labels without a click handler, Line and Shape not enlarged.
  - Verify: player — the rule exists only inside the coarse-pointer media query; a Button gets it, an unbound Label does not; an `<input>` is wrapped. Browser (T31's session, device emulation): a tap 7 % outside a Button's edge clicks it, recorded with coordinates.

- [ ] **T23 — DataGrid remote binding renders a row window** (Q4; depends on WASM-005's frame)
  - Files: `crates/cobolt-web-player/src/view/data.rs`.
  - Do: a DataGrid bound to IndexedFile or SQL asks for a window of rows and the next one on scroll, never the whole table; in-memory `Rows` unchanged.
  - Verify: player `view::data::` — with a stub backend of 100,000 rows the view requests only `(offset, count)` windows and renders the window it got; in-memory tests unchanged.

- [ ] **T24 — Bundled fonts and the 50 % warning** (Q5)
  - Files: `crates/cobolt-web-bundle/src/lib.rs` (`warnings`), `crates/cobolt-web-player/src/base_css.rs` (`--pr-font`).
  - Do: bundle as `@font-face` the face the desktop uses for its built-in family (`crates/cobolt-forms/src/fonts.rs::is_builtin`) and every family a form names that the project supplies under `Assets/`; skip an OS-only family and name it in `warnings`; warn when font files exceed 50 % of the application's size, suggesting system fonts only; `--pr-font` names the bundled family first, then the system stack.
  - Verify: `cargo test -p cobolt-web-bundle fonts` — built-in face and an `Assets/` font copied; an OS-only family skipped and named; the warning present at 51 % and absent at 49 %. Browser: the computed font of a Label is the bundled family (no fallback).

## IDE half (R5, R6, R12)

- [ ] **T25 — Check reports class D controls and operations** (R5, AC4)
  - Files: `crates/cobolt-project-tools/src/web/classes.rs` (new), `web/mod.rs`, the Check and Build tools in `crates/cobolt-project-tools/src/tools/` (`build.rs` already maps `WebBuildError::Refused`).
  - Do: over every `.wcfrm`/`.mcfrm`, one line per class D control (its line in the form file) and per handler use of a class D operation of a multi-class control (`VWR::Print()`, a ToolBar button with `run-app:`), in the existing `file:line: construct — alternative` format, alternative from T12's row; run in Check and before Build.
  - Verify: `cargo test -p cobolt-project-tools web::classes` — a `.wcfrm` with a `Custom` plug-in control and a handler calling a Viewer's `Print` gives two lines with the right line numbers (AC4); the same form as `.cfrm` gives none.

- [ ] **T26 — Toolbox hides class D on Web and Mobile** (R5, R6, AC4)
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs` (`ToolboxPanel::show` filters `TOOLS` by project kind), `i18n.rs` (the note).
  - Verify: IDE `toolbox` — on a Web and a Mobile project no listed type is class D, on a Desktop project the list is unchanged; the existing test that every `ControlType::ALL` has a `TOOLS` row stays green; the event-binding editor is untouched (no diff in its file).

- [ ] **T27 — Properties panel hides `NotOnWeb`, marks `Approximate`** (R6, AC5)
  - Files: `crates/cobolt-ide/src/panels/properties.rs` (`show_selection`, `:4094`), `i18n.rs` (tooltip prefix).
  - Do: on a web or mobile form a `NotOnWeb` row is not drawn and its value is not touched; an `Approximate` row stays editable with the note in its tooltip.
  - Verify: IDE `properties` — a web form omits `OsBlur` and shows `RotateX` with the approximate tooltip; **AC5**: a desktop form carrying every property (approximate and hidden ones included) saved and reloaded as `.wcfrm` keeps every value, compared property by property.

- [ ] **T28 — Agent kit reads the table** (R2, R12)
  - Files: `crates/cobolt-ide/src/agent.rs` (the control list given to agents).
  - Verify: IDE `agent` — the list for a Web project contains no class D type and names the D operations of Maps/Viewer/ToolBar; Desktop unchanged. Golden prompts regenerated with `COBOLT_WRITE_GOLDEN=1` only if they change, diff reviewed.

- [ ] **T29 — System KB tables and `chunked.data`** (R7, R12, AC8)
  - Files: `crates/cobolt-compiler/src/lib.rs` (`control_structure` `:7797`, `control_purpose` `:7809`, `controls_reference_doc` `:8630`, the usage notes), `assets/knowledge/chunked.data`.
  - Do: each control's web class and per-operation exceptions, generated from T12's table; web-only members: none exist yet (WASM-007 adds `CssClass`/`Style` with its own KB rows). Regenerate with `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-compiler controls_reference` (every `ALL` type has a class line); IDE `prebuilt_chunked_kb_matches_the_published_documentation` green (AC8).

- [ ] **T30 — i18n** (spec §6)
  - Files: `crates/cobolt-ide/src/i18n.rs`.
  - Do: new `Tr` fields in six languages (en, es, pt-BR, fr, ja, zh): the toolbox note that unavailable controls are hidden for this kind, the "approximate on the web" tooltip prefix, the Check message frame.
  - Verify: IDE `no_empty_ui_translations` and a new `the_web_control_strings_are_in_every_language`.

## Browser and desktop comparison (R11; AC2, AC3, AC6, AC7, AC9–AC13)

- [ ] **T31 — Layers end to end in Chrome** (AC9–AC13; after WASM-010 T12 emits A9)
  - Do: a layered reference form generated from COBOL: stacking (AC9); start hidden, then `SET … Visible TO TRUE` shows it on the next frame (AC10); a click at an opaque backdrop point discarded, at a transparent one reaching the control beneath (AC11); Tab through base then each visible layer (AC12); a DataGrid at a rounded corner, a pixel read outside the arc shows the form background (AC13).
  - Verify: numbers written here (element at point, event logs, focus sequence, pixel values).

- [ ] **T32 — Property round-trip for every class A and B control** (R11, AC2)
  - Do: the browser counterpart of `tests/controls/`' `assert_control`: each property set to a distinct value through the host API and read back from the DOM; the expected element per class A row of T12's table. Tooling runs outside the repository (PRIME DIRECTIVE); the Rust side is a gallery `.wcfrm` and a player test that lists the expected element per control.
  - Verify: per control, properties set / read back / mismatches, written here; zero mismatches outside the documented differences.

- [ ] **T33 — Desktop comparison harness** (R4, R8, R11, AC3, AC6)
  - Files: a native test in `cobolt-forms` (`render` feature, headless) producing per-control geometry, sampled colours and event logs for the reference form; the browser run produces the same records.
  - Do: one reference form with at least one control of each class; fixed data for every class B control; a scripted click, change, focus and key on each class A control. Compare geometry per control, colours for class B, event order for class A. Every difference found is listed per control (R4) and is the only accepted one.
  - Verify: the comparison output written here; AC3 and AC6 met when only listed differences remain.

- [ ] **T34 — Keyboard-only walk and accessibility audit** (R10, AC7)
  - Do: a keyboard-only walk through the reference form that reaches and operates every control; axe-core run from the headless browser by tooling outside the repository.
  - Verify: the walk's sequence and axe's result (no critical issue) written here.

## Finalize

- [ ] **T35 — Finalize** (2.x.x line: no documentation task; `README.md` and `docs/` are never touched)
  - Bump `z` once in `crates/cobolt-ide/src/version.rs`; one `CHANGELOG.md` entry above the previous heading, dated, saying "Written on features-2x".
  - Run the tests of the crates touched, `--no-fail-fast`, reading every `test result` line: `cargo test --no-fail-fast -p cobolt-forms --features render`, `-p cobolt-wasm-abi`, `-p cobolt-web-bundle`, `-p cobolt-project-tools`, `-p cobolt-compiler`, `-p cobolt-ide --bin cobolt-ide`, and `cargo test --no-fail-fast --manifest-path crates/cobolt-web-player/Cargo.toml --lib`.
  - Verify: all green; the credential scan of the staged diff is clean.

## Done criteria

Every acceptance criterion is covered by a task's verification: **AC1** T1, T12 · **AC2** T32 · **AC3** T33 · **AC4** T25, T26 · **AC5** T27 · **AC6** T33 · **AC7** T34 · **AC8** T29 · **AC9** T18, T31 · **AC10** T18, T31 · **AC11** T19, T31 · **AC12** T20, T31 · **AC13** T18, T31. Today only AC1 is met. Q2 stays open with the working assumption above.
