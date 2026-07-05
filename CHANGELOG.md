<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerRustCOBOL — Changelog

## [PowerRustCOBOL 1.27.88] — 2026-07-04

### Fixed

- **Transparency / "growing frame with no content" over inner controls inside databound repeating GroupBox cards (ControlArray) when the parent Panel is scrolled.**
  The symptom (visible in screenshot.png): rounded card frames appeared and moved on scroll, but Labels, Buttons, PictureBoxes and bound data inside the cards were missing or only partially visible; the card's own gradient/fill showed through as if a clip rect or mask was applying full transparency over the children. Upper cards sometimes showed partial data; scrolled-in cards showed "[Loading...]" at bottom of the empty frame area.
  Root cause: in `render_form`, control `screen` rects for cards and members were correctly shifted by `ancestor_auto_scroll_offset` (`-scroll`), and `picturebox_container_border` subtracted scroll for `_ContainerClip`. However, the axis-aligned `clip` passed to `painter.with_clip_rect(...)` (and thus to `draw_control`) was always built from `containers::clip_rect` at raw form-space positions (`origin + cm`) with no scroll adjustment. For a label inside a card inside a VScroll Panel, the card's `content_rect` contribution to clip stayed at its laid-out y while the label drew at y - scroll → draw happened outside the active clip → nothing (or only bg) rendered inside the moved card.
  Fix (minimal, unified engine only):
  - Added `ancestor_clip_rect` (modeled on the existing ancestor scroll walk): walks parents, subtracts scroll *only* for non-scroller ancestors (the repeating cards live in scrolled content space); keeps scroller Panel clips fixed so content does not escape the viewport.
  - Updated `picturebox_container_border` to skip the scroll subtraction when the *immediate* parent itself carries HScroll/VScroll (correct fixed border clip for direct PictureBox children of a rounded scrolling Panel).
  - The general clip site in the render loop now calls the new helper.
  Affects Preview and Run Form (both use `render_form`). Designer canvas (`render_faces`) is unaffected (scroll always zero). Databinding, expansion, PlacementEffect, and H/VScroll drive are unchanged. Backward compatible.
  Guardrail note: per AGENTS.md DataGrid guardrail (this fixes databound repeating visual "cards" that were designed to act like databound lists/grids, plus render/clip/rounded/embedded controls in `render.rs`), the datagrid-quality checklist was applied (see below).

## [PowerRustCOBOL 1.27.87] — 2026-07-04

### Fixed

- **Databound repeating GroupBox (ControlArray) now shows per-row data on cards (not clones of the first/template).**
  - Codegen now emits `INVOKE array 'RefreshBinding'` during `COBOL-DATA-BINDINGS-POPULATE` for ControlArray targets. Combined with load order (LOAD before POPULATE), this automatically computes live ItemCount from the table, recreates the N visual instances, re-applies PlacementEffects, *and* pushes the current row values into each instanced member's properties.
  - Runtime hydration in `refresh_control_array_binding` now also directly pushes StateUpdates under the exact instanced ids (`Group.Group-N.Member`) in addition to the indexed Member path. Guarantees `RunState` / `live(instanced)` in render sees distinct per-card values even across timing or id-resolution edges.
  - Setting `ItemCount` on a `_BindingArray` group now auto-rehydrates current table rows (hook in `obj_set`).
  - Cards with data now appear on initial form load / after RefreshBinding exactly as requested.

## [PowerRustCOBOL 1.27.86] — 2026-07-04

### Fixed

- **"index out of bounds: the len is 26 but the index is 26" crash when running a form containing a databound repeating GroupBox (ControlArray) + ~26 total controls.**
  Root cause: `render_form` performed live+`expand_repeating_groups` producing an expanded control list, then passed indices from its `render_order` (into the expanded list) to `picturebox_container_border`, which always indexed `input.controls` (the original designed list, len=26). Instanced members therefore OOB'd when looking up their (instanced) parent's border for `_ContainerClip`.
  Fix: `picturebox_container_border` now receives the effective `controls` slice + `&dyn FormState` explicitly. Callers in `render_form` (post-expand) and `render_faces` (unchanged) pass the right list. Parent lookup and clip now work for cards inside rounded repeating groups.
  Also removed last stray `[RUN-FORM-DATABIND]` eprintln (converted to targeted tracing debug).

- **RefreshBinding on databound ControlArray/GroupBox-2 now fully recreates cards, reapplies PlacementEffect, and hydrates member values.**
  `refresh_control_array_binding` now:
  - Sets live `ItemCount` (drives re-expansion in render using live state).
  - Bumps `_BindSeq` (forces appear-clock key change).
  - Re-hydrates every mapped member prop for 1..N from the current COBOL table rows (via `_BindingMappings` seeded at codegen/launch time + `set_member_indexed` + subscript values) so cards show fresh data.
  - Stamps `_CardEffect` / `_Card*` metadata during next `expand_repeating_groups` (using live props).
  The appear clock key now incorporates N + seq so deployment (Deal/FadeIn) replays on refresh exactly like first load.
  Codegen emits the `_BindingMappings` seed for ControlArray targets; IDE launch path seeds it too.

- **No more 0-instanced cards after SEED with positive ItemCount on nested or top-level databound GroupBox-2.**
  (Follow-up to prior live_controls + removal of parent guard; expansion now consistently produces instances for IsRepeatingGroup + ItemCount>0.)

## [PowerRustCOBOL 1.27.85] — 2026-07-04

### Fixed / Diagnostics

- **Data-binding instrumentation restricted to run-form execution only (no RAD/designer noise).**
  Removed unconditional `[DATABIND]` output from designer apply/seed/refresh, preview row helpers, canvas ghosts, render expand, and codegen.
  Focused debug now only in the run-form path: interpreter `binding_load`/`binding_populate`/`refresh_datagrid_binding` + REFRESHBINDING (emits `[RUN-FORM-DATABIND]` on stderr during actual "Run Form").
  Includes note highlighting that ControlArray/GroupBox databind has no auto member hydration in `POPULATE` (unlike DataGrid's `_Binding*` + refresh path). Use the same data source on datagrid-1 vs groupbox-2 and observe the difference at runtime.
  Tracing debug remains available with `COBOLT_LOG=debug`. No behavior changes.

## [PowerRustCOBOL 1.27.84] — 2026-07-04

### Fixed

- **Arrayed-control (repeating GroupBox) data binding now shows each row at runtime.**
  A `Member(idx)::Prop` write no longer drops its subscript: `StateUpdate` carries a
  1-based `instance_index`, the interpreter tags array-member writes with it, and the
  IDE routes each write to the matching cloned card. Cloned ids use a collision-safe,
  group-prefixed scheme (`<group>.<group>-<n>.<member>`) shared by the renderer, the
  preview seed, and the runtime router.
- **Numeric expressions parse in the last remaining position.** DISPLAY/MOVE/IF/
  subscripts already accepted arithmetic; the screen-position phrase now accepts a
  bare `LINE`/`COL` (no leading `AT`) with expression operands, e.g.
  `ACCEPT ITEM LINE A + B COL C + D`.

### Changed

- **Repeating-group cards: layout, indexing, and empty-source behavior.** Instances
  start at index 1 and are placed by direction — full height+spacing down (Vertical)
  or width+spacing across (Horizontal), Grid wrapping every `ItemsPerRow`. A databound
  group with **0 rows renders no card at all**; an unbound group still shows its
  template.
- **New `PlacementEffect` for card appearance** (repeating GroupBox): `None` (instant),
  `Deal` (all cards start stacked on the first card, then deal out to their final spots
  one after another — off-screen cards are placed instantly, no phantom fly-in), or
  `FadeIn` (each fades in at its final spot, 200 ms, one after the previous finishes).

## [PowerRustCOBOL 1.27.83] — 2026-07-03

### Fixed

- **Control Array (repeating GroupBox) databinding now produces instances.** When a
  GroupBox is marked IsRepeatingGroup and bound as ControlArray to a CobolTable (or
  other) source, `apply_data_binding_target_properties` + `seed` now correctly drive
  `ItemCount`/`PreviewItemCount` from `OCCURS` (preferred) or row count. Preview render
  snapshots controls *after* seeding so `expand_repeating_groups` (and designer ghosts)
  see n>1. `PreviewState::live` and designer ghost clones now inject per-instance
  `#N` values for mapped member controls (including non-default props like ImagePath,
  Checked via extra writes + updated `preview_value_key`). Added richer `[DB-ARRAY]`,
  `[DB-ARRAY live]`, `[DESIGNER-DB]` instrumentation. Unmapped source fields no longer
  affect general binding or count logic (mappings are a subset). DataGrid and scalar
  bindings unaffected.
- **Deleting a databound control no longer leaves an orphaned binding that blocks
  Run.** When a control (or an array member/host GroupBox) is deleted, its data
  binding — and any dangling field mapping — is pruned automatically. Forms whose
  orphan predates this are self-healed: the binding is dropped before the guardian
  runs, so a since-deleted target no longer triggers `missing-target-control`.
- **The data-binding editor reopens a control's saved configuration.** A control
  that is already bound now offers "Edit current binding", and re-selecting its
  saved source pre-fills the source selection, field rows, and (for control arrays)
  the field→member mappings — instead of starting blank every time.

### Changed

- **Slider: Fore color drives the knob, Back color the track body.** The Appearance
  section's Fore/Back colour now tints the thumb and the track along the scale
  (overriding the Liquid Glass default only when set to a non-default colour). The
  legacy Track/Thumb/Fill colour pickers — which the renderer never used — were
  removed from the inspector.
- **COBOL-table binding no longer asks for a separate occurs item.** A 01-level
  table with OCCURS is enough; the occurs item is derived from the selected 01
  automatically, so the redundant (read-only) occurs-item field is gone.

## [PowerRustCOBOL 1.27.82] — 2026-07-03

### Added

- **Neumorphic form properties (illumination & shadows).** When Theme=Neumorphic the
  Form Properties panel (Appearance section) now shows style-specific editors:
  gradient colors for the illumination effect and for the shadow effect; separate
  blur strength sliders for each; transparency intensity; distance (shadow offset);
  tint color + line weight + blur strength for the extra 3-sided border
  (top-right → bottom-right → bottom-left). All are per-form, stored in .cfrm,
  round-tripped, and affect every render surface. Other themes are unaffected.
  Defaults preserve the previous recipe look.

### Fixed

- **Neumorphic illumination no longer darkens the highlight sides.** The gradient
  color lerp ignored the stops' alpha, so a "transparent" stop (stored as
  transparent black) dragged the highlight toward black — the top/left edges and
  top-left corner rendered dark, like a second shadow. The RGB lerp is now
  alpha-weighted and the layer opacity scales with the interpolated stop alpha, so
  transparent stops fade the effect out instead of darkening it.
- **Neumorphic illumination color pickers were missing from the Appearance grid.**
  A bare separator consumed the first cell of the two-column grid, shifting the
  "Illum. grad." row's pickers into a clipped third column. The separator now
  occupies its own full row.
- **Neumorphic tinted rim now lands on the corner junctions.** The extra 3-sided
  border is drawn as a single connected polyline that begins at the 45° midpoint
  of the top-right corner arc and ends at the 45° midpoint of the bottom-left arc,
  following each corner's own radius (± the per-layer blur offset) — it no longer
  passes half a corner, wraps onto the left edge, or leaves square smudges from the
  old rectangular top/left masks. The outer contour and inner bevel share the same
  path so all three accents stay aligned.

## [PowerRustCOBOL 1.27.81] — 2026-07-03

### Fixed

- **Procedural Neumorphic (soft-UI) effect now fully functional.** The four-layer relief
  (very light neutral bg, raised panel, opposite soft shadows via translate+expand
  rounded rects for highlight top-left / shadow bottom-right, plus two subtle inset
  inner rims) is implemented following the reference recipe. Uses only egui 0.29
  drawing primitives. When Neumorphic glass style is active the form page defaults
  to the recipe bg (#ECEFF4) unless the designer set a distinct colour. Buttons
  suppress incompatible specular overlays. All forms tests and renders remain
  pixel-parity compliant.
- **Charts adapt to the Neumorphic style.** Light chart face (instead of the dark
  navy glass face), soft pastel data palette, faint gray-blue grid/axis lines,
  gray-blue badge/hint text, an engraved inner "tray" contour on the card, molded
  (gently domed) pie slices and bars, white sector separators, and a soft drop
  shadow under the pie disc. The preview and Run-Form viewports also switch to
  light soft-UI widget visuals with gray-blue text (the glass near-white text was
  invisible on the light surface). The dual soft shadows are now truly
  directional — highlight up-left, shadow down-right — instead of a uniform halo.
- **Charts: `BarCornerRadius` is honoured again.** The bar chart's corner-radius
  property existed in the inspector but the renderer hardcoded the radius; both
  the flat and gradient bar paths now apply it (clamped per bar).

## [PowerRustCOBOL 1.27.80] — 2026-07-03

### Changed

- **Neumorphic theme is now 100% procedural — no images.** Neumorphic is a third
  surface style alongside Classic/Enhanced Liquid Glass: elements share the
  background colour and "emerge" from it via a dual soft shadow (dark toward the
  bottom-right, light toward the top-left), with no frost and no hard border.
  Selecting it sets the glass style and clears any image theme-pack override, so
  the neumorphic look no longer loads PNG assets.

### Fixed

- **Forms/DataGrid: rounded corners render correctly while running, even nested.**
  A DataGrid with a corner radius rendered square: its opaque cell/row fills and
  its straight outer-border lines painted over the rounded background, and the
  corner-notch mask (used for Panels) skips nested containers. The grid's own
  fills are now clamped to the grid rect and rounded at the bottom corners (the
  header already rounds the top), and the outer border is drawn as an inset
  rounded stroke — so nothing square pokes past the radius and the outline no
  longer bleeds a light rim outside the corner.

## [PowerRustCOBOL 1.27.79] — 2026-07-03

### Fixed

- **IDE: the running form now runs in its own process.** "Run Form" no longer
  drives the form's interpreter and viewport inside the IDE's own event loop;
  it spawns an isolated `rcrun run-form-ipc` child that hosts the interpreter and
  talks to the IDE over a framed IPC channel (stdin/stdout). A busy or spinning
  form can no longer peg the IDE's UI thread — the IDE stays responsive while the
  form window carries its own cost.
- **Forms/DataGrid: appearance background now rules the whole grid interior.**
  Regions with no explicit cell/column colour — the gap around a framed "pill"
  cell, the filler area right of the last column, and the gutter beneath the
  vertical separators — now fall back to the DataGrid's appearance
  `BackgroundColor` instead of showing the translucent glass (which read as a grey
  wash over the form backdrop). A fully-transparent column colour is treated as
  "unset" so the fallback applies.
- **Forms/DataGrid: column background image honours its configured opacity.** The
  per-column background image was painted at a fixed alpha; it now scales by the
  column's "Cell background" opacity and the control's own Opacity.
- **Forms/DataGrid: rounded corners are kept while running.** Opaque cell/row
  fills no longer poke a square corner past the grid's rounded background — the
  corner-notch mask now trims the DataGrid the same way it trims Panels/GroupBoxes.
- **Forms/Panel: rounded corners keep their border line.** The corner-notch mask
  repainted the backdrop over each rounded corner, erasing the container's
  border/rim there (border visible on the straight edges, missing at the corners).
  The outline is now restored on all four corner arcs.

### Changed

- **Themes: asset-based theme packs.** Added a Neumorphic form theme option and
  bundled theme assets (updated cobalt-steel control skins; new emerald-glass and
  neumorphic packs).
- **Chore: removed the temporary `[TIMER-DBG]`/`tdbg` diagnostic instrumentation**
  left over from the idle-CPU investigation.

## [PowerRustCOBOL 1.27.78] — 2026-07-01

### Fixed

- **IDE/Forms: precise Timer-driven repaint scheduling.** Live forms and Timers no longer poll the repaint loop at a fixed fraction of the interval. The Timer now wakes the UI exactly when the next `onTick` is due (with a small floor to avoid spin), and the running-form viewport removes its unconditional `request_repaint()`. Combined with the prior reactive root loop, idle forms no longer peg a CPU core.

## [PowerRustCOBOL 1.27.77] — 2026-07-02

### Fixed

- **Parser: `FUNCTION RANDOM` now parses, and the FUNCTION-argument loop can no
  longer hang.** `RANDOM` lexes as a keyword (from `ACCESS MODE IS RANDOM`), so
  the FUNCTION-name reader rejected it and left the token stuck — inside another
  function's arguments (e.g. `FUNCTION INTEGER(FUNCTION RANDOM * 4)`) that spun
  the parser forever and froze the IDE. The intrinsic name is now accepted and
  the argument loop has a no-progress guard, so malformed input always terminates
  with a diagnostic.
- **Parser: optional `IS` before `GLOBAL` / `EXTERNAL`.** The COBOL-85 `[IS]
  GLOBAL` / `[IS] EXTERNAL` connective is now consumed instead of warning.
- **Forms: DataGrid scrolling no longer bleeds into its container.** While the
  pointer is over a DataGrid the grid consumes the wheel and zeroes the frame
  scroll deltas, so the surrounding GroupBox / form no longer scrolls too.
- **Forms: a Timer honours its `Enabled` property.** The tick is gated on the
  Timer's own `Enabled` property (default true), not the generic control-enabled
  chrome flag, so a non-visual Timer with `enabled="false"` still fires `onTick`.
- **IDE: reactive repaint loop.** A running form no longer pegs a CPU core while
  idle — the event loop repaints only when there is work to drain and sleeps
  otherwise.

### Added

- **IDE: event-handler validation with the project-tree semaphore.** Each form's
  generated COBOL is validated (syntax + semantic) on save, on Run, before Build,
  and on project open; the tree dot turns green/red per form and Run/Build are
  refused with a clear message until the code is fixed.
- **IDE: apply runtime DataGrid layout back to the design.** While a form runs, a
  floating "Apply layout to design" button persists interactively-adjusted column
  widths / row height into the form as the control's new defaults.
- **IDE: Run-Form process inspector.** A toolbar toggle (in the designer RAD
  toolbar, next to Run Form) opens an always-on-top window with real-time line
  charts (Process CPU, Memory RSS, Child processes, System Memory), a process
  tree, and leak / runaway-CPU / rogue-subprocess detection that dumps to the
  console and a per-project-configurable file. Samples only while the Live
  Interpreter runs. (Adds the `sysinfo` dependency.)

## [PowerRustCOBOL 1.27.76] — 2026-07-02

### Fixed

- **`FUNCTION RANDOM` now honours its seed argument (COBOL-85).** The intrinsic
  previously ignored any argument, so the standard way to seed the generator —
  `FUNCTION RANDOM(seed)` — did nothing and every run replayed the same
  sequence. A seed argument now (re)seeds the generator deterministically and
  returns the first value of that sequence, while an unseeded `FUNCTION RANDOM`
  continues the current sequence. The same seed reproduces the same sequence
  (e.g. `FUNCTION RANDOM(12345)` for stable demo data); seed from a varying
  value for a fresh sequence each run (e.g. `ACCEPT ws-time FROM TIME` then
  `FUNCTION RANDOM(ws-time)`).
- **`ACCEPT … FROM TIME` resolves to real centiseconds.** The TIME register uses
  the standard `HHMMSSss` 8-digit layout, but the hundredths were hard-coded to
  `00` (whole-second resolution). They are now populated from the sub-second
  clock — still COBOL-85 compliant (hundredths of a second, not milliseconds).
  This also sharpens the time portion of `FUNCTION CURRENT-DATE` and lets a
  time-seeded `FUNCTION RANDOM` differ between runs launched more than ~1/100 s
  apart.

## [PowerRustCOBOL 1.27.75] — 2026-07-02

### Fixed

- **Non-visual controls (Timer) can no longer freeze the IDE/RAD.** The form
  interpreter now honours a cooperative cancellation flag checked between every
  statement — which covers every PERFORM iteration and paragraph body — so a
  long-running or looping event handler (for example a `Timer` `onTick`, or a
  heavy `onLoad`) aborts promptly instead of pinning the interpreter thread.
  Closing the running-form window, relaunching, or exiting the IDE now sets that
  flag and no longer blocks the UI thread: `stop()` waits only a short bounded
  grace period for the thread to unwind and then detaches it, so the application
  stays responsive and is always closeable. A blocking statement (e.g. a large
  file read) can finish its current step, but can never hang the whole IDE.
- **Timer tick coalescing.** A `Timer` emitted `onTick` on every elapsed
  interval regardless of whether the previous tick's handler had finished; a
  handler slower than the interval flooded the unbounded UI→interpreter event
  queue, starving the quit sentinel and eventually hanging a relaunch. Ticks are
  now skipped while the interpreter's event queue is still non-empty
  (WinForms-style coalescing), while user events — clicks, edits, focus changes,
  quit — are never dropped.
- **COBOL errors surface in a dialog and stop cleanly, without closing the
  IDE.** A parse/semantic (syntax) error when launching a form, or a fatal
  runtime error reported by the interpreter, is now shown in a modal "⛔ COBOL
  error" window (with the message and a pointer to the Output panel) in addition
  to the console line. Processing stops and the IDE/RAD stays open — it no longer
  fails silently or leaves the run window in limbo.
- **Find bar no longer drifts while searching.** The editor's floating Find/
  Replace bar was anchored to the scrolling text-content rect, so jumping
  between matches (which scrolls the editor) dragged the bar up and down. It is
  now anchored to the stable editor viewport and stays where it opened; it is
  also draggable — move it anywhere and it keeps that position.

## [PowerRustCOBOL 1.27.74] — 2026-07-01

### Fixed

- **Editor search has a case-insensitivity toggle** — the code editor's Find bar
  now has an "Aa" toggle (on by default) to switch between case-insensitive and
  case-sensitive matching. Matching also switched to ASCII-lowercasing so match
  offsets stay byte-accurate.

## [PowerRustCOBOL 1.27.73] — 2026-07-01

### Fixed

- **Controls can be renamed from the Properties inspector** — the control id in
  the Identity header is now an editable field. Renaming to a unique, valid
  identifier updates every reference form-wide: child `parent` links, `LabelFor`
  associations, the control's event-handler paragraph names, data-binding
  target/source/member references, and control references in handler/procedure
  code (`Old::…` / `Old(i)…`). The rename is undoable; a taken or invalid name is
  rejected.

## [PowerRustCOBOL 1.27.72] — 2026-07-01

### Fixed

- **DataGrid frozen columns also clip the filter row** — with frozen columns and
  column filters shown, horizontally scrolling drew the scrollable columns' filter
  input boxes *over* the frozen band. The filter inputs (egui widgets, not
  painter-drawn) are now clipped to the region right of the frozen columns, so the
  whole filter row scrolls behind the frozen columns like the header and body.

## [PowerRustCOBOL 1.27.71] — 2026-07-01

### Fixed

- **Editor Find box keeps focus while typing** — typing in the code editor's
  Find field no longer kicks focus back into the editor after each keystroke.
  Incremental search still scrolls to the first match, but keyboard focus only
  moves into the editor on an explicit navigation (Next/Prev/Enter) or after
  applying an autocomplete suggestion. The Replace field was unaffected.

## [PowerRustCOBOL 1.27.70] — 2026-07-01

### Fixed

- **Repeating groups now render their instances at run time** — the shared render
  engine expands a repeating GroupBox into N cards (one per `ItemCount`, falling
  back to `PreviewItemCount`), laid out by the group's Vertical / Horizontal /
  Grid direction and spacing. Each instance's controls are cloned with
  instance-unique ids so they render and interact independently. This is the
  runtime foundation for control-array data binding (data-driven population of
  each card is the next step).

## [PowerRustCOBOL 1.27.69] — 2026-07-01

### Fixed

- **Array-member event handlers receive the array index** — a control that
  belongs to a repeating group (array) now gets an event-handler stub that
  declares `01 CONTROL-ARRAY-INDEX PIC S9(4) COMP-5.` in its LINKAGE SECTION and
  `PROCEDURE DIVISION USING CONTROL-ARRAY-INDEX`, with a hint showing indexed
  member access (`Name(CONTROL-ARRAY-INDEX)::Property`). Both the generated
  `.cbl` stub and the handler skeleton opened in the IDE editor use it; regular
  (non-array) controls keep the plain stub.

## [PowerRustCOBOL 1.27.68] — 2026-07-01

### Fixed

- **Repeating-group binding editor can map fields to member controls** — the
  control-array (repeating GroupBox) binding modal now has a "Map fields to
  controls" section: each source field can be assigned to a member control, and
  the control's default bindable property is shown and applied (Label→Caption,
  TextBox→Text, CheckBox→Checked, **PictureBox→ImagePath**, ComboBox/ListBox and
  numeric controls→Value). Applying the binding records a `ControlProperty`
  mapping per mapped field; unmapped fields are skipped.

## [PowerRustCOBOL 1.27.67] — 2026-07-01

### Fixed

- **DataGrid frozen panes can cast a drop shadow** — a new "Frozen pane shadow"
  toggle (on by default) draws a soft shadow from the last frozen column
  (rightward) and the frozen header/rows (downward) onto the content that
  scrolls behind them, giving the usual spreadsheet freeze cue. The shadow only
  appears when the grid actually scrolls in that direction.

## [PowerRustCOBOL 1.27.66] — 2026-07-01

### Fixed

- **Data-binding source buttons work for a repeating GroupBox (control array)** —
  choosing a source (COBOL table, SQL, …) on a repeating GroupBox did nothing:
  the editor was keyed by the array **name** rather than the GroupBox's control
  id, so the settings modal opened and instantly closed, and apply couldn't
  resolve the target. The binding editor is now keyed by the control id, so the
  modal stays open and the binding applies to the control array.

## [PowerRustCOBOL 1.27.65] — 2026-07-01

### Fixed

- **DataGrid Image cells support corner radius and a drop shadow** — a column
  whose Edit control is **Image** now exposes an "Image corner radius" and an
  "Image drop shadow" setting in the column editor. The cell picture is rounded
  to the chosen radius and, when enabled, drawn over a soft two-layer shadow.

## [PowerRustCOBOL 1.27.64] — 2026-07-01

### Fixed

- **DataGrid frozen columns now clip the scrollable columns** — with one or more
  frozen columns, horizontally scrolling the grid drew the scrollable columns
  *over* the frozen band. Scrollable header cells, body cells, and column
  separators are now clipped to the region right of the frozen columns, so they
  slide behind the frozen band (mirroring the already-correct frozen-row
  behavior).

## [PowerRustCOBOL 1.27.63] — 2026-07-01

### Fixed

- **DataGrid COBOL masks now honour edited pictures** — a column mask such as
  `ZZZ,ZZZ,ZZ9.99` now zero-suppresses, inserts digit-group commas and the
  displayed decimal point, and signs negatives, so a bound `S9(9)V99` value like
  `000003000.00` renders as `3,000.00` instead of the raw zero-padded digits.
  Check-protection (`*`) fill and `9(n)`/`S9(n)V99` plain pictures are unchanged.
- **DataGrid columns can render their value as an image** — a new **Image** edit
  control treats the (alphanumeric) cell value as an image file path and draws
  the picture fitted to the cell (falling back to the path text when the image
  can't be loaded), useful for thumbnail columns.

## [PowerRustCOBOL 1.27.62] — 2026-07-01

### Fixed

- **Run Form no longer fails silently on a startup error** — a runtime error
  while a form starts (e.g. in its `onLoad`) was swallowed, so the interpreter
  thread died and the run window never appeared with no message at all. Fatal
  form-runtime errors are now surfaced to the Output pane
  (`⛔ Form runtime error: …`) so the cause is visible instead of the run
  silently doing nothing.
- **Clearer error when assigning to a control method** — using a method call as a
  MOVE/assignment target (e.g. `MOVE … TO Grid::RefreshBinding()`) now reports
  which method it was and that it must be called as a statement, not used as a
  receiving field.

## [PowerRustCOBOL 1.27.61] — 2026-07-01

### Fixed

- **Data-bound DataGrid COBOL mask can be changed and is applied** — a COBOL
  mask typed into a bound column's editor was reset to the bound field's
  PICTURE on every save/run binding refresh, so it could never be changed and
  cell values did not pass through it. The binding refresh now seeds a column's
  mask from the field only when the column has none, preserving a user-typed
  mask as a deliberate override; the DataGrid renderer already formats each
  bound value through that mask before display.

## [PowerRustCOBOL 1.27.60] — 2026-07-01

### Fixed

- **DataGrid alternating highlight can now stripe columns** — a new "Alternating
  mode" setting (Rows / Columns / None) chooses whether the alternating
  background color highlights every other row (default, unchanged for existing
  forms), every other column, or nothing. Column striping reuses the same
  alternating color and opacity and sits beneath any per-cell or per-column
  background.

## [PowerRustCOBOL 1.27.59] — 2026-07-01

### Fixed

- **DataGrid background patterns tile evenly** — dot, stripe, cross, X, X-dots,
  and O background patterns previously started from a fixed top-left offset and
  left a ragged, uneven gap at the right and bottom edges. Patterns now pick the
  tile count that fits the grid and spread the tiles with balanced margins on all
  sides, so the automatic tiling looks evenly distributed at any size.

## [PowerRustCOBOL 1.27.58] — 2026-07-01

### Fixed

- **Every control is fully Liquid Glass again** — the solid background layer
  added in 1.27.57 flattened glass-backed controls (buttons, PictureBoxes,
  menu/tool bars) into opaque slabs. That underlay is removed from the shared
  glass renderer, so all controls return to translucent Liquid Glass. The one
  exception is the DataGrid, which keeps fine-grained control over its grid,
  column, row, and cell backgrounds: a DataGrid still on the default background
  renders as glass, and a chosen grid background color paints solid beneath the
  frost.
- **DataGrid grid-line color is now the grid's foreground** — the Appearance
  section's Fore color drives the DataGrid grid-line color, replacing the
  separate entry in the grid settings modal. A grid left on the default
  foreground uses the subtle built-in line color, and existing forms with a
  `GridLineColor` continue to render via a compatibility fallback.

## [PowerRustCOBOL 1.27.57] — 2026-07-01

### Fixed

- **Control background opacity can now reach true solid colors** — glass-backed
  controls paint their selected background color as an opacity-aware base layer,
  and custom interactive backgrounds no longer cap full opacity below 100%.

## [PowerRustCOBOL 1.27.56] — 2026-07-01

### Fixed

- **DataGrid column filters are now editable in the header** — filter rows use
  real text inputs instead of painted placeholder text, and edits update the
  same `AdvancedGrid`/`ColumnFilters` metadata used by DataGrid filtering.

## [PowerRustCOBOL 1.27.55] — 2026-07-01

### Fixed

- **DataGrid inner shape colors can now be driven by cell values** — the
  DataGrid column settings modal exposes value/color definitions for inner
  shapes, allowing values such as `ACTIVE`, `SUSPENDED`, and `CANCELED` to map
  to their own shape background colors.

## [PowerRustCOBOL 1.27.54] — 2026-07-01

### Fixed

- **DataGrid data-binding debug output no longer floods the console** — removed
  temporary `[data-binding]` console diagnostics from binding apply and shared
  DataGrid render paths while leaving binding hydration and preview rows
  unchanged.

## [PowerRustCOBOL 1.27.53] — 2026-07-01

### Fixed

- **DataGrid frozen panes and keyboard navigation now work in the shared
  renderer** — frozen columns/rows use the resolved advanced grid state,
  scrollable rows no longer displace the frozen row band, keyboard movement
  selects cells with arrows/Page/Home/End, column resize booleans honor typed
  values, explicit text alignment wins, and grid/row/column backgrounds support
  cross, X, X-dots, and O patterns.

## [PowerRustCOBOL 1.27.52] — 2026-07-01

### Fixed

- **DataGrid headers and COBOL masks now render correctly** — DataGrid headers
  apply `CornerRadius` only to the top-left and top-right corners, and bound
  columns now use their COBOL mask when formatting displayed cell values.

## [PowerRustCOBOL 1.27.51] — 2026-06-30

### Fixed

- **DataGrid settings moved into a focused modal and rendering options now apply
  in the shared renderer** — the right-side properties pane now exposes a compact
  DataGrid editor entry, while the modal handles grid backgrounds, column masks,
  edit controls, column fonts, filter headers, inner cell frames, gauges, and
  line styles without forcing minimum modal dimensions.

## [PowerRustCOBOL 1.27.50] — 2026-06-30

### Fixed

- **Advanced DataGrid behavior is now guarded across runtime, binding, CSV,
  i18n, and docs** — DataGrid runtime methods, CSV export mode/order,
  advanced binding metadata preservation, localized property labels, and the
  English developer guide now cover the advanced grid feature set.

## [PowerRustCOBOL 1.27.49] — 2026-06-30

### Fixed

- **Runtime controls now honor `CornerRadius` in custom interactive renderers** —
  runtime-only drawing paths for DataGrid, ListBox, NumericUpDown, TabControl,
  TreeView, Splitter, MenuBar, ToolBar, StatusBar, and Button hover/press
  overlays now use the same corner-radius helper as the Form Designer.

## [PowerRustCOBOL 1.27.48] — 2026-06-30

### Fixed

- **DataGrid rows now stay inside the grid and scroll** — the shared renderer
  clips DataGrid content to the control bounds, keeps the header fixed, supports
  mouse-wheel scrolling through overflowing rows, and draws a small scrollbar
  indicator when additional rows are available.

## [PowerRustCOBOL 1.27.47] — 2026-06-30

### Fixed

- **DataGrid alternating row highlight is subtle by default** — added
  `AlternatingRowOpacity` with a 20% default, applied it in the shared renderer,
  exposed it in the DataGrid properties panel, and included it in format-painter
  style copying.

## [PowerRustCOBOL 1.27.46] — 2026-06-30

### Fixed

- **DataGrid cells now clip text to their own columns** — long bound values
  such as thumbnail image paths no longer spill across column separators and
  visually overlap adjacent captions in the shared form renderer.

## [PowerRustCOBOL 1.27.45] — 2026-06-30

### Fixed

- **DataGrid now exposes `RefreshBinding()` for live COBOL tables** — running
  forms seed bound DataGrid metadata into the interpreter, the runtime
  `RefreshBinding` method rebuilds `Rows` from current `FIELD(n)` COBOL table
  values, and the editor autocomplete now lists the method for DataGrid
  controls.

## [PowerRustCOBOL 1.27.44] — 2026-06-30

### Fixed

- **DataGrid COBOL table bindings now read indexed MOVE initialization rows** —
  bound grids now hydrate rows from form event, control event, and user
  procedure statements like `MOVE value TO FIELD(n)` before falling back to
  synthetic preview data, so COBOL table examples populated in OnLoad/OnShow
  display their real row values.

## [PowerRustCOBOL 1.27.43] — 2026-06-30

### Fixed

- **DataGrid bindings now hydrate preview rows** — DataGrid binding refresh now
  fills the grid's `Rows` property from COBOL table initial values when
  available, falls back to deterministic preview rows from binding fields when
  only definitions exist, and refreshes bound grid properties before save/run so
  existing bindings do not stay header-only.

## [PowerRustCOBOL 1.27.42] — 2026-06-30

### Fixed

- **DataGrid binding diagnostics now expose the render gap** — applying a
  DataGrid binding now writes renderer-compatible `Name:Type` column
  definitions, the Properties panel edits those definitions as multiline text,
  and Apply/render paths emit console diagnostics for columns, rows, data
  source, and binding field counts.

## [PowerRustCOBOL 1.27.41] — 2026-06-30

### Fixed

- **Data binding Apply now hydrates DataGrid basics** — applying a DataGrid
  binding updates the grid's Columns and DataSource properties from the binding
  definitions immediately, and replaces the previous binding for that target so
  the visible grid stays wired to the latest settings.

## [PowerRustCOBOL 1.27.40] — 2026-06-30

### Fixed

- **COBOL table Add field now chooses from missing real fields** — the COBOL
  table binding editor shows a selector of fields that exist in the selected
  working-storage table but are not yet mapped, and hides the add flow once all
  table fields are present.

## [PowerRustCOBOL 1.27.39] — 2026-06-30

### Fixed

- **COBOL table data binding now uses real working-storage tables** — the table
  selector no longer invents a placeholder value, lists eligible 01-level
  GLOBAL OCCURS tables from the form working-storage section, limits added
  fields to missing fields from the selected table, and shows an explicit
  dropdown settings button only for Dropdown edit controls.

## [PowerRustCOBOL 1.27.38] — 2026-06-30

### Fixed

- **Data-binding source fields use aligned grid columns again** — source-field
  rows now render through an egui grid while dropdown details remain in their
  separate modal, keeping columns aligned without reintroducing inline
  dropdown-detail width pressure.

## [PowerRustCOBOL 1.27.37] — 2026-06-30

### Fixed

- **Dropdown configuration now opens in its own modal** — selecting Dropdown
  for a data-binding field or clicking an existing dropdown row opens a separate
  configuration window, keeping source-field rows compact and avoiding inline
  dropdown-detail width pressure.

## [PowerRustCOBOL 1.27.36] — 2026-06-30

### Fixed

- **Dropdown configuration panels no longer widen the source-field grid** — the
  expanded data-binding dropdown editor stays aligned under the Picture column
  while using a bounded in-row panel instead of forcing horizontal scrolling.

## [PowerRustCOBOL 1.27.35] — 2026-06-30

### Fixed

- **Data-binding settings no longer auto-grow beyond the working area** — the
  modal width is capped and wide source-field grids scroll horizontally inside
  the window instead of forcing the data-binding window wider than the screen.

## [PowerRustCOBOL 1.27.34] — 2026-06-30

### Fixed

- **COBOL table data-binding settings now open a real configuration form** —
  selecting COBOL table shows the table and occurs item, COBOL field mappings,
  nested dropdown lookup configuration with COBOL/indexed origins, add/restore
  behavior, and COBOL-table Apply validation inside the data-binding modal.

## [PowerRustCOBOL 1.27.33] — 2026-06-30

### Fixed

- **REST API data-binding settings now open a real configuration form** —
  selecting REST API shows endpoint, method, headers, authentication, JSON
  preview, JSONPath guidance, REST field mappings, add/restore behavior, and
  REST-specific Apply validation inside the data-binding modal.

## [PowerRustCOBOL 1.27.32] — 2026-06-30

### Fixed

- **SQL data-binding settings now match the reference form details** — SQL
  pagination uses the requested navigation glyphs, dropdown lookup mock data
  uses the current Indexed-file samples, nested dropdown panels include the
  separator styling, and Apply validation rejects non-positive lookup line
  limits.

## [PowerRustCOBOL 1.27.31] — 2026-06-30

### Fixed

- **Data Binding settings now include the SQL control configuration form** —
  selecting SQL opens an interactive SQL-control source section with paginated
  result-set preview controls, SQL field mappings, dropdown lookup
  configuration for SQL controls and COBOL tables, line limits, add/restore
  behavior, and Apply validation.

## [PowerRustCOBOL 1.27.30] — 2026-06-30

### Fixed

- **Data Binding settings now open a full Indexed file configuration modal** —
  the Properties panel opens an interactive, scrollable editor with source
  selection, clear confirmation, indexed-file preview pagination, sample record
  grid, source-field mapping rows, dropdown sub-configuration panels, restore
  removed fields, and Apply validation.

## [PowerRustCOBOL 1.27.29] — 2026-06-30

### Fixed

- **Data Binding source buttons now open a configuration editor** — choosing
  Indexed, SQL, COBOL table, REST, or Agent AI in the Properties panel opens an
  inline binding editor for the selected approved target, allowing the developer
  to review and edit binding IDs, source details, fields, and generated mappings
  before applying the form-level binding.

## [PowerRustCOBOL 1.27.28] — 2026-06-29

### Fixed

- **Data binding is now guarded from source to runtime** — form-level bindings
  can wire Indexed files, SQL, COBOL tables, REST schemas, and Agent AI
  structured outputs into grids, charts, dropdowns, listboxes, and explicit
  control arrays, while the Data Binding Guardian blocks unsafe saves, runs,
  checks, builds, and packages before mappings can corrupt bound data.
- **Bound controls keep writeback state recoverable** — generated binding code
  loads and populates targets deterministically, writable bindings preserve row
  identity and pending edits, read-only bindings never write back, and failed
  updates keep the pending value available for repair.

## [PowerRustCOBOL 1.27.27] — 2026-06-29

### Fixed

- **Run Form property updates now treat quoted and bare property names the
  same** — live interpreter updates such as `MOVE Slider-1::Value TO
  label-5::Caption` now overwrite the designed `Caption` property instead of
  creating a separate uppercase `CAPTION` shadow key, matching the behavior of
  `label-5::"Caption"`.

## [PowerRustCOBOL 1.27.26] — 2026-06-29

### Fixed

- **Run Form now fires the newly exposed live control events** — the unified
  form renderer emits right-click/context-menu, double-click alias, mouse move,
  mouse wheel, hover enter/leave, control load, TextBox text/key aliases,
  checkbox/radio value aliases, and Slider final value events, and a regression
  test verifies generated `onClick` handlers execute through the live
  interpreter channel.

## [PowerRustCOBOL 1.27.25] — 2026-06-29

### Fixed

- **Designer clipboard actions are now reachable from the RAD UI** — Cut, Copy,
  Paste, and Duplicate are available in the Form Designer toolbar and the canvas
  right-click menu, using the same selection-aware clipboard behavior as the
  keyboard shortcuts.

## [PowerRustCOBOL 1.27.24] — 2026-06-29

### Fixed

- **Existing controls now show the expanded Events list** — the Properties
  panel already reads events dynamically from each control type, and those
  supported event lists now include the comprehensive design-time events such as
  `onRightClick`, `onDoubleClick`, `onHoverEnter`, `onResize`, and
  `onPropertyChanged` while preserving non-visual control event lists.

## [PowerRustCOBOL 1.27.23] — 2026-06-29

### Fixed

- **Reusable User Controls are now project-backed designer components** — a
  selected GroupBox can be saved as a named User Control, shown in the Toolbox,
  deployed as regular qualified controls, nested inside other User Controls, and
  removed from the project without breaking existing form instances.
- **Designer clipboard workflows are safer and more complete** — `Cmd+C`,
  `Cmd+X`, `Cmd+V`, and `Cmd+D` now copy, cut, paste, and duplicate selected
  controls while preserving child containment and regenerating IDs/handlers for
  pasted instances.
- **Deletion confirmation now protects event-handler code** — removing controls
  with handler bodies shows a confirmation dialog with handler/control counts,
  while confirmed deletions still recycle the removed code for recovery.
- **User Control child properties and events resolve by qualified IDs** —
  selecting a deployed User Control shows grouped child properties, runtime
  `GetProperty`/`SetProperty` can target `Child.Property`, and generated event
  dispatch keeps full child IDs such as `CUSTOMERCARD-1-BUTTON1--ONCLICK`.

## [PowerRustCOBOL 1.27.20] — 2026-06-26

### Fixed

- **Child controls no longer bleed past a rounded container's corner** — a
  PictureBox, Animator, chart, or any control inside a rounded GroupBox/Panel is
  now clipped to the parent's **border path** instead of its own bounds. The
  control keeps its size; whatever overflows the container's rounded corner is cut
  by the container shape. The render engine widens a child's clip to the parent
  border and the unified `draw_control` rounds each face (image, film, glass card,
  chart background) on any corner that lands on the container arc.
- **Corner-notch masking for content egui can't round-clip** — egui only supports
  axis-aligned clipping, so grid lines and other fine chart/control content can't
  be rounded directly. After a rounded container's children are painted, its four
  corner notches are repainted with the backdrop (solid colour and/or the
  background image, tiled when the form tiles), covering any residual bleed. The
  solid fill is applied only when opaque, so a translucent canvas is never
  double-painted into a darker wedge.
- **`draw_glass` is now per-corner** — the frosted-glass card renderer accepts a
  full `Rounding` (not a single radius), so a control whose corner meets a rounded
  container follows that arc on that corner alone. Images load with Repeat wrap so
  a tiled backdrop also tiles inside the notch mask.

## [PowerRustCOBOL 1.27.19] — 2026-06-24

### Fixed

- **GroupBox no longer clips child content under an empty title area** — a
  GroupBox always reserved an 18px top "caption band" (plus a 6px inset on the
  other sides) for its content, so a child placed across the top edge was cut off
  even when the GroupBox had no caption. Children are now clipped to the border on
  every side, and the top caption band is reserved only when a caption is actually
  shown (sized to clear the legend text). With a caption, content clips just below
  it; without one, it reaches the border.

## [PowerRustCOBOL 1.27.18] — 2026-06-24

### Fixed

- **Captions only where they belong** — non-text controls (Panel, and any control
  without a real caption) no longer show a centered "<id>" placeholder. The
  **GroupBox** caption now renders as a title on the **top-left border** (classic
  legend look, editable in the property pane) instead of centered. Label, Button,
  CheckBox, and RadioButton keep their caption.
- **Form/image "Browse" button now sets the path in the window you clicked** — the
  background-image picker (and the control image picker) used shared keys across
  the in-window inspector and a detached Designer window, so whichever rendered
  first consumed the file-dialog result and the path didn't land where expected.
  The picker state is now namespaced per window.

## [PowerRustCOBOL 1.27.17] — 2026-06-24

### Fixed

- **PictureBox image no longer dimmed in the running form / preview / binary** —
  a photo shown vivid on the designer canvas looked washed-out everywhere else,
  because the runtime surfaces drew images through a different code path with a
  different tint. They now use the designer's exact path, so an image looks the
  same on every surface.

### Changed

- **The unified render engine now drives all four surfaces** — the Form Designer
  canvas, the live preview, the running form, and the compiled binary all render
  through one engine in `cobolt-forms`. The four separate, drifting draw loops
  (and the old `render_run_control`) are gone, so the designer, preview, run, and
  binary are guaranteed to match. The designer keeps its editor overlay
  (selection handles, badges, drop hints) on top. This completes the unification
  begun in 1.27.16.

## [PowerRustCOBOL 1.27.16] — 2026-06-23

### Fixed

- **Compiled binaries now look like the IDE** — a standalone built form rendered
  every control as a plain, unstyled native widget (no background image, no glass
  charts, no themed slider/date picker). The compiled binary now draws through the
  same unified render engine as the Form Designer, live preview, and running form,
  so a packaged form matches what you designed.
- **Slider no longer gets stuck in a built binary** — a freshly opened window
  could receive a burst of phantom pointer input that left the slider's drag in a
  bad state, so dragging the knob did nothing. Phantom input at window-open is now
  ignored during a short warm-up, and the slider clears any stale drag state, so it
  responds normally.
- **Chart "Data Binding — COBOL Table" properties stack one per row** — the table
  binding fields (Table item, Row count item, Label field, Value field(s), Series
  labels) were packed onto a single line, forcing the property pane to scroll
  horizontally. Each field is now on its own row. The Scatter chart's "Bubble size
  field" had the same defect and is fixed too.

### Changed

- **One render engine for every surface (internal)** — the Form Designer canvas,
  live preview, running form, and compiled binary now share a single rendering
  engine in `cobolt-forms`, replacing four separate draw loops. This is the
  groundwork that makes the designer, preview, run, and binary look identical.

## [PowerRustCOBOL 1.27.15] — 2026-06-23

### Fixed

- **A control's value now reaches its change handler in the running form** —
  binding a COBOL handler to a control's change event (e.g. a Slider's) and
  reading the control's value inside it returned the initial value (`0`) no
  matter how far you moved the knob. UI-driven value changes (slider drag, text
  edit, combo/list selection) are now synced to the interpreter the instant the
  event fires, so the handler reads the live value — not the seeded default.

## [PowerRustCOBOL 1.27.14] — 2026-06-21

### Fixed

- **Rotated lines no longer clip in the running form** — the running form clipped
  every control to its own bounding box, so a Line rotated past its (thin) box was
  cut off, while the designer drew it in full. The run now clips controls only to
  their container ancestors (like the designer), so a rotated/angled Line shows
  completely on every surface.
- **Line DashStyle works** — the Line control ignored DashStyle and always drew a
  solid line. **Dash**, **Dot**, and **DashDot** now render real dashed/dotted
  patterns (via egui dashed-line shapes); **Solid** is unchanged.

### Added

- **Line rounded ends** — a **Rounded ends** toggle on the Line control draws
  round caps at both endpoints.

### Fixed

- **PictureBox image aspect ratio** — the Form Designer canvas stretched the
  image to fill the box regardless of **SizeMode**, while the preview and running
  form preserved the aspect ratio. The designer now honours SizeMode too (using
  the image's native size), so Fit/Zoom/Center keep the aspect ratio and the image
  looks identical on the canvas, in the preview, and at run time.
- **Chart axis lines are controllable** — charts drew fixed X/Y axis lines with no
  way to remove them. All (non-pie/donut) charts now have **Show X axis line** and
  **Show Y axis line** toggles so the chart can show just its data.
- **Line direction is no longer limited to the presets** — the **Line** control
  gained an **Angle°** property (0–359) so it can point in any direction, not only
  Horizontal/Vertical/Diagonal. Setting the angle overrides the legacy preset;
  existing lines are unchanged. *(An on-canvas rotation knob is a follow-up.)*

## [PowerRustCOBOL 1.27.12] — 2026-06-21

Fix: apply the active theme to the preview & running-form viewports (spec 017).

### Fixed

- **Charts/themed controls use the same theme on every surface** — the live
  preview and the running form each render in their own egui `Context`, and only
  the designer canvas was calling `set_active_theme`. So `draw_chart_preview`
  (which reads the active theme for its palette/styling) fell back to defaults in
  the preview and run, making charts look different from the canvas. Both now set
  the owning designer's active theme pack on their context before rendering.
- Removed the temporary on-screen render diagnostics.



Fix: charts render through the designer's path on every surface (spec 017 step).

### Fixed

- **Charts match the designer in preview and the running form** — the live
  preview and the running form drew charts by calling the chart painter
  **directly**, bypassing the card-frame + glass layering that `draw_control`
  applies on the designer canvas, so a chart (e.g. an AreaChart) looked washed
  out / different when run. Both now render charts through **`draw_control`** — the
  exact path the Form Designer uses — so the chart is identical on the canvas, in
  the preview, and in the running form. (Part of the spec-017 move toward a single
  rendering engine; the Form Designer is the source of truth.)

## [PowerRustCOBOL 1.27.10] — 2026-06-21

Fix: running form now matches the designer/preview (backdrop + glass).

### Fixed

- **Glass toggle tracked reliably in the running form** — the run resolved the
  owning designer only by file path, which could miss (path normalisation) and
  fall back to a stale launch-time value, so a glass-on chart rendered dim
  (solid dark) instead of vivid (frosted). It now resolves by path **then form
  name** and keeps the runtime snapshot in sync, so the running form's glass
  matches the canvas — charts and other translucent content look identical.
- **Running form backdrop** — the live (interpreted) form derived its background
  straight from the form colour, so a pure-black / unset background rendered the
  window pure black. The preview and designer instead fall back to a default dark
  navy in that case. The run now uses the **same** rule (strip `#`, first 6 hex
  digits, black/unset ⇒ dark navy), so translucent glass content — charts in
  particular — no longer looks washed out over a black window and matches the
  canvas and preview.

## [PowerRustCOBOL 1.27.9] — 2026-06-21

Universal corner radius + rounded content (spec 016). Pre-production, treated as
a fix.

### Added

- **Corner radius on every bordered control** — buttons, text boxes, combo/list
  boxes, picture boxes, data grids, numeric/date pickers, progress bars, sliders,
  shapes, charts, and the containers now share one **Corner radius** property.
  The background and border round to it, and content is clipped to the rounded
  shape — a **PictureBox** image is trimmed to the rounded corners over any
  background (via a textured `RectShape`), and chart frames round too.
  `Corner radius = 0` keeps square corners and no clipping (the default, so
  existing forms are unchanged); the value is clamped to half the smaller side.
  Applies identically on the canvas, the live preview, and the running form.

### Changed

- The container property is unified under **CornerRadius**; the legacy
  `BorderRadius` is still read as an alias so older forms round correctly.

### Known limitations

- The editable text/scroll layer of run-time inputs stays square inside a rounded
  frame, and container **children** are clipped to the rectangular content area
  (egui has no rounded scissor; rounded corners are cosmetic on the frame).

## [PowerRustCOBOL 1.27.8] — 2026-06-21

Fix: preview/run rendering parity with the Form Designer.

### Fixed

- **Glass look now matches the designer** — the live preview and the running form
  always rendered with the Liquid-Glass look, even when the designer's glass
  toggle was off. They now mirror the launching designer's glass setting, so a
  flat (non-glass) canvas runs flat — charts and panels keep the same vivid,
  non-frosted appearance instead of looking washed out.
- **Containers (Panel) render in the live run** — a Panel previously fell through
  to a generic blue glass box with a "Panel" caption when the form was run; it now
  uses the shared `draw_control` renderer, so it looks identical to the designer
  (and to GroupBox). The generic run-time fallback for any other visual control
  also routes through `draw_control` instead of an approximate glass box.
- **TextBox look matches everywhere** — the live preview and the running form drew
  TextBoxes with a hard-coded dark-blue glass and fixed light text. They now draw
  the same `draw_control` face as the designer (honouring BackgroundColor /
  gradient / border) with the editable text in the control's ForegroundColor.
- **DateTimePicker field matches** — the running form's date field now uses the
  shared renderer for its face (the calendar popup is unchanged).
- **Containment in the running form** — the live run now clips children to their
  container's content area, fades them by ancestor opacity, and hides controls on
  a non-selected tab page, exactly like the designer and preview (e.g. a chart
  inside a GroupBox no longer spills past the box). The running form also tracks
  the designer's glass toggle live.

## [PowerRustCOBOL 1.27.7] — 2026-06-21

Visual repeating groups (GroupBox arrays) — spec 015, Phases 1–2 (designer +
model only). Pre-production, treated as a fix.

### Added

- **GroupBox appearance** — new **Hide caption**, **Hide background**, and a
  two-colour **Background gradient** (Vertical / Horizontal / DiagonalDown /
  DiagonalUp / Radial) alongside the existing background colour and border
  radius. Hide-background draws no fill/border while children stay visible.
- **Repeating groups** — a GroupBox can be marked as a repeating array template
  via the right-click menu (**Set / Unset as Repeating Group**). A **▦ ARRAY**
  badge marks it, and a **Repeating Group** properties section exposes array
  name, item count, data source, layout direction (Vertical / Horizontal /
  Grid), item spacing, items-per-row, auto-scroll-parent, clone-events and
  preview-items.
- **Design-time preview** — **Preview items > 1** renders render-only ghost
  instances laid out per the chosen direction, without adding controls to the
  form model (selection/undo unaffected).

Runtime instancing, indexed event dispatch, and data binding (spec 015 Phases
3–5) are not included in this release.

## [PowerRustCOBOL 1.27.6] — 2026-06-21

Fixes: form-designer scrolling regression + chart monochrome polish.

### Fixed

- **Form Designer scrolling restored** — the canvas `ScrollArea` now uses
  `auto_shrink([false, false])`, so a form larger than the viewport scrolls again
  (regressed alongside the spec-012 container work).
- **Monochrome colour picker** — compact 16×16 grid with **1px pure-white internal
  grid lines**, no external border and no padding between swatch and line (much
  smaller than before); the selected swatch is marked.
- **Greyscale column** — one hue column of the 256-colour selector is replaced by
  **16 shades of grey** (still no pure black/white).
- **Chart "Hide Background" honoured** — a chart with `HideBackground` set now
  draws **no** card/glass frame at all. Previously the generic control frame was
  painted behind the chart preview, so the background still showed through when
  the property was checked.

### Added

- **Monochrome gradient** — a `MonochromeGradient` toggle on charts. Each data
  element gets its **own** tonal gradient (±20% of the base): bars shade
  vertically, scatter bubbles and pie/donut slices shade radially; line and area
  charts get a **vertical** gradient fill (bright at the line, fading toward the
  baseline). Area/stacked translucency for the non-gradient case is unchanged.
- **Smooth line/area curves** — the `Smooth` chart property now actually renders
  a **Catmull-Rom spline** (line and area/stacked charts), matching the smooth
  reference look; `ShowPoints` gates the line markers.

## [PowerRustCOBOL 1.27.5] — 2026-06-20

Fix: **chart monochrome mode** (spec 013). Pre-production, treated as a fix.

### Fixed

- Charts gain a **Monochrome** toggle + a **MonochromeColor** chosen from a fixed
  **256-colour** selector (pure black/white and near-extremes excluded).
- When on, data elements (bars, slices, lines, points, areas, markers) render in
  distinguishable **tonal variations** of the base colour (same hue family) across
  all six chart types; grid lines use a soft **pastel** variant, axes a stronger
  pastel, and slice borders a lighter variant — so the chart isn't flat.
- Labels, legends, titles keep the **foreground colour** (not recoloured), and
  area/stacked **transparency** is unchanged.
- When off, charts render exactly as before. Grid visibility remains the existing
  **ShowGridLines** toggle (no duplicate property added).

## [PowerRustCOBOL 1.27.4] — 2026-06-20

Fix: **form container controls** — GroupBox, Panel, and TabControl become real
containers (spec 012). Pre-production, so treated as a fix that completes intended
behaviour.

### Fixed

- **Real containment & nesting** — controls can be placed inside GroupBox, Panel,
  and TabControl to any depth and in any combination, via a `parent` link on each
  control. The `.cfrm` round-trips it; legacy `<Children>` files are migrated on
  load, and the old Panel `Scrollable` flag maps to the new `AutoScroll`.
- **Reparent by drag-and-drop** — drop a control on the form to detach it, over a
  container's content area to nest it, or over another control to adopt that
  control's parent. Undoable, with a guard against dropping a container into its
  own descendant.
- **Move-with-parent & cascade delete** — moving a container moves its whole
  subtree; deleting a container removes its descendants.
- **Clipping + border radius** — children are clipped to the container's content
  area; each container has a configurable `BorderRadius`.
- **Working `Opacity`** — a container's `Opacity` now fades the container and its
  subtree (the property previously had no visual effect on any control).
- **TabControl pages** — each tab owns its own children; clicking a tab switches
  the active page; only the active page's controls are shown and interactive — at
  design time and in the IDE run-preview.
- **Auto-scroll property** — per-container `AutoScroll` (default off → overflow is
  clipped), editable in the properties pane.

Known follow-ups (spec 012): auto-scroll *scrollbars*, the drag-time drop-target
highlight, and standalone-binary render parity.

## [PowerRustCOBOL 1.27.3] — 2026-06-20

Fix: chart controls gain a **Hide background** property.

### Fixed

- Every chart (Bar, Line, Pie, Area, Scatter, Donut) now has a **Hide
  background** toggle in the properties pane. When checked, the panel's
  background fill and border frame are not drawn — only the chart content (grid,
  axes, labels, data) is rendered, so the chart sits transparently on the form.
  Default is off (unchanged appearance). Applies at design time and at run time
  (shared renderer).

## [PowerRustCOBOL 1.27.2] — 2026-06-20

Fix: complete the RustCOBOL `::` member-access model — IntelliSense now lists
properties alongside methods, the `::` operator chains to any depth over a real
nested object model, and a control property is a receiving field for every verb
(spec 011).

### Fixed

- **IntelliSense `::` popup** — the property/method list now shows **properties
  (green)** as well as methods (light blue); the `::` / `::"` member list and
  chain tails (`…)::`, `…::member::`) are resolved against the chain's root
  control.
- **Member-access chains** — the `::` operator now chains to any depth with one
  consistent syntax: `Grid-1::Rows(I)::Columns(2)::Value`,
  `obj::Value::toUpperCase()`. A `(n)` subscript indexes a collection, a bare
  name is a property, and `()` is a method call.
- **Nested object model** — controls hold nested objects and indexable
  collections (rows → columns → cells, list items), navigated by the chain;
  legacy newline-string item lists interoperate (`List-1::Items(3)`).
- **Property as a receiving field for every verb** — not just `MOVE`/`SET` but
  `STRING`/`UNSTRING INTO`, `ADD … TO`, `COMPUTE`, `ACCEPT`, `INSPECT`,
  `INVOKE … RETURNING`, … may write to `control::property` (and nested cells).
- **Collection / value helpers** on a chain element — `Count`, `Delete`,
  `Clear`, `Add`, and the transforms `toUpperCase`, `toLowerCase`, `trim`, `len`.
- **INITIALIZE on a control** — `INITIALIZE obj` resets its `Value` property;
  `INITIALIZE obj::prop` targets one property; `INITIALIZE obj name` initialises
  each operand by its own rules.
- A chain ending in a **method call** `()` is a value, never a receiving field —
  `MOVE name TO obj::method()` is rejected (runtime error + a compile-time
  diagnostic); a chain ending in a **property** or **indexed cell** is assignable.

## [PowerRustCOBOL 1.27.1] — 2026-06-20

Fix: standardise control property & method access on the RustCOBOL `::`/`INVOKE`
forms and remove the redundant Fujitsu `"Property" OF Control` syntax (spec 010).

### Changed / Fixed

- **One way to touch a control property** — the `::` member syntax and the
  `INVOKE` verb, for both read and write:
  - GET: `control::property`, `control::"property"`, `INVOKE control "property"
    RETURNING x`, `INVOKE control "GET-property" RETURNING x`.
  - SET: `MOVE v TO control::property`, `SET control::"property" TO v`,
    `INVOKE control "property" USING v`, `INVOKE control "SET-property" USING v`.
  - A bare member resolves as a property accessor (get with no argument, set with
    a `USING` argument); `GET-`/`SET-` are explicit prefixes; explicit methods
    (`SetCaption`, `GetText`, …) keep priority.
- **Case-insensitive property names**, and numeric properties read as numbers so
  `IF Slider1::Value > 50` and arithmetic stay algebraic.
- **Removed** the inherited Fujitsu `"Property" OF Control` syntax entirely
  (parser, AST, runtime, IntelliSense, docs). No legacy code used it, so this is
  not a breaking change. (This also drops the `OF` form's property-as-receiver in
  arbitrary verbs and indexed property paths; use `::`/`INVOKE` with a data item.)
- **IntelliSense:** typing `::` or `::"` after a control id lists its **properties
  (green)** and **methods (light blue)** and filters as you type; a lone `"` opens
  no popup.

## [PowerRustCOBOL 1.27.0] — 2026-06-19

Form module model (spec 009) — procedure scoping, sharing & lifecycle.

### New / Changed

- **All procedures are `IS COMMON`.** Every woven procedure — event handlers and
  user procedures alike — is now generated `IS COMMON PROGRAM`, so any procedure
  is callable from anywhere in the form module (a handler may `CALL` another
  handler, a user procedure may call a handler, …). Previously only user
  procedures were `COMMON`.
- **Static-by-default procedures.** A procedure's local `WORKING-STORAGE` is now
  initialised **once** and **persists across calls** (re-entering a handler keeps
  its values; exiting does not cancel it), matching COBOL-85. `CANCEL "<name>"`
  resets a procedure's state; `INITIALIZE` (unchanged) resets the items you
  choose, each call.
- **`FD … IS GLOBAL`.** A global `FD` is accepted and validated; the file and its
  record are visible to the form's nested procedures. `GLOBAL` placement is now
  validated (valid only on `01`/`77` items and `FD`s) alongside `EXTERNAL`.
- **Inline `obj::method()` as a value.** The inline method call now works as a
  value operand inside `DISPLAY`/`MOVE`/`COMPUTE` (e.g. `DISPLAY S::len()`), not
  only as a statement — folded in from the 005 Rust-FFI AC6.

### Notes

- `INVOKE-FORM` (form invoking another form) and `#INCLUDE` (copying in external
  embedded programs) are **deferred** to a follow-up; cross-process `EXTERNAL`
  sharing remains scoped to a single run unit.

## [PowerRustCOBOL 1.26.0] — 2026-06-19

Form themes (spec 007) — engine + selection + reference pack.

### New

- **Selectable form themes.** Forms can be skinned by a selectable, extensible
  catalogue of themes, applied by the shared control renderer so a themed form
  looks identical in the designer, the preview, and (once the web target lands)
  the compiled app. Two kinds sit under one picker: the built-in procedural
  **Liquid Glass** (the default, unchanged) and **asset-pack** themes.
- **Project default + per-form override.** A project default theme is set in
  *Settings → Appearance* (`[forms] theme` in `cobolt.toml`); any form can
  override it in its *Appearance → Form theme* property, or inherit the default.
  Resolution is per-form → project → Liquid Glass. (i18n across all six
  languages.)
- **Self-describing asset packs (9-slice).** A theme pack is a drop-in folder
  `assets/themes/<id>/` with a `theme.toml` manifest plus per-control /
  per-state 9-slice images, an optional themed background, a foreground colour,
  and a chart palette/stroke. New packs are discovered automatically and appear
  in the picker with no code change. A control a pack doesn't cover falls back to
  Liquid Glass; a control's explicit colours still win.
- **Themed charts.** Pie/line/bar data marks take the active theme's palette and
  stroke, not just the chart frame.
- **Optional themed background.** A form can opt into a pack's background image
  (*Appearance → Use theme background*); otherwise its own back-colour / image
  applies.
- **Reference pack `cobalt-steel`.** A small, procedurally generated, original
  pack (see `cargo run -p cobolt-forms --example gen_reference_theme`) that
  exercises the engine end-to-end.

### Changed

- **Unified control renderer.** The canonical `draw_control` (and the system-font
  module) now live in `cobolt-forms` (`cobolt_forms::paint`), so the designer,
  preview, run form, and future compiled/web binaries all draw through one
  renderer. Liquid Glass is byte-for-byte unchanged.

### Notes

- The four "special" art packs (stainless steel, dark wood, modeling clay,
  knitted wool) and the WASM/desktop-binary embedding are staged behind their
  asset and spec-006 dependencies; the engine is ready for both.

## [PowerRustCOBOL 1.25.0] — 2026-06-18

COBOL Structure & shared data (spec 005, Phase 1).

### New

- **COBOL Structure editor.** The form inspector lists the five shared COBOL
  blocks — `SPECIAL-NAMES`, `REPOSITORY`, `FILE-CONTROL`, `FILE SECTION`,
  `WORKING-STORAGE` — plus the form's user procedures; clicking a row opens a
  popup that edits that one block. Add / rename / delete user procedures from the
  list. The blocks are woven verbatim into the generated program. (i18n across
  all six languages.)
- **`GLOBAL` / `EXTERNAL` / `GLOBAL EXTERNAL` data sharing.** `EXTERNAL` `01`/`77`
  items (and `FD`s) are now shared run-unit-wide by their real name; `GLOBAL`
  items stay visible to a module's contained programs. The checker flags
  `EXTERNAL` on anything other than `01`/`77`/`FD`.
- **User procedures.** Named nested programs the event handlers can `CALL`;
  generated `IS COMMON` so siblings may call them.
- **COBOL-2002 `USAGE IS OBJECT REFERENCE <class>`** parses, and `REPOSITORY`
  starts pre-seeded with a curated Rust-FFI type bridge (all primitives + common
  std classes, `CLASS RUST-x IS "Rust.x"`). Declarations generate today; invoking
  Rust through them is Phase 2.

## [PowerRustCOBOL 1.24.1] — 2026-06-18

### Fixed

- **`EXTERNAL` data is now shared run-unit-wide.** `01`/`77`-level items (and
  `FD`s) declared `EXTERNAL` were silently ignored at run time. They are now
  registered in a single run-unit store and shared by their real name across
  program activations, so one program's update is seen by another in the same
  run unit. `GLOBAL`-only items stay private to each form, as before. (spec 005)

## [PowerRustCOBOL 1.24.0] — 2026-06-17

Per-control test example projects, plus form-rendering fixes surfaced by them.

### New

- **Per-control examples** — a runnable test project for every toolbox control
  under `examples/<control>/`: the subject control with a console
  `DISPLAY "<Event> working"` per supported event and one button per property
  that changes it from COBOL via `INVOKE … "SetProperty"`. `examples/build-all.sh`
  builds all 34; `cargo run -p cobolt-codegen --example check_examples` verifies
  event/property coverage.

### Fixed

- **Codegen** — Timer (`SetInterval`), DataGrid (`ExportCSV`), and AgentObject
  (`Ask`) emitted `INVOKE "<id>" '…'` with the control id quoted as a string
  literal, which the parser rejected; the id is now an unquoted identifier so
  forms using those controls build.
- **Run-form window** — scrollbars now appear automatically when a form is larger
  than its window, so off-screen content is reachable.
- **Default colours** — `ForegroundColor` now defaults to white, and Button/Label
  text falls back to white, so captions are legible on the dark run-form canvas.

## [PowerRustCOBOL 1.23.0] — 2026-06-15

Indexed File Editor, Grid Browser, and `.cidx` codegen in the IDE.

### New

- **Indexed Files** project-tree category (after Forms) listing `.cidx` definitions.
- **Indexed File Editor** — separate viewport to define or inspect record layout,
  keys, storage flags, and per-field grid controls; structural lock after finalize.
- **Import existing…** — register an on-disk indexed data file; schema inferred via
  `inspect_any_path` when available.
- **Indexed File Grid Browser** — virtualized table with add/edit/delete,
  Commit/Rollback, and schema-drift protection.
- **Codegen** — `generated/<stem>-indexed.cbl` regenerated on Build / Run / Debug /
  Check (same contract as forms).
- **`cobolt-indexed` crate** — `.cidx` XML model shared by IDE, codegen, and runtime.

## [PowerRustCOBOL 1.22.0] — 2026-06-14

Branding, About box, generated-code lifecycle, and spec-driven development infrastructure.

### New

- **Application icon.** The IDE ships with the PowerRustCOBOL samurai icon
  (`assets/images/powerrustcobol-icon.png`), used as the window/taskbar icon and
  overridable via an `app-icon.png` in the config directory.
- **Help → About.** A new About window shows the mascot, version, copyright and
  the Apache-2.0 license.
- **"Powered by PowerRustCOBOL" badge.** A badge (`made-with-powerrustcobol.png`,
  plus a high-resolution `.webp` master) with README + Developer's Guide
  instructions for developers to add it to their own apps' About box.
- **Developer banner in generated COBOL.** Every RAD-generated `.cbl` now opens
  with a `*>` comment block telling the developer it is generated, must not be
  edited directly, and may change structure between versions.
- **Automatic regeneration.** Form COBOL is regenerated from the current forms
  on every **Build / Run / Debug / Check**, so what compiles and runs always
  matches the forms.
- The mascot now appears in the README and the Developer's Guide cover.

### Infrastructure

- **Spec-driven development.** Gated workflow (`/specify` → `/plan` → `/tasks` →
  `/implement` → `/docsync`) with steering docs, templates, and committed skills
  under `specs/` and `.claude/skills/`. See `specs/README.md`.

## [PowerRustCOBOL 1.21.0] — 2026-06-14

French interface language.

### New

- **French (Français) UI language.** A sixth interface language joins
  EN/ES/PT/JA/ZH; pick 🇫🇷 Français from the language selector. The full IDE UI
  is translated (menus, toolbar, settings, the form designer and property
  inspector, the debugger, the AI assistant, and the documentation viewer).
  - The Documentation viewer shows the English Developer's Guide for French
    until a French translation of the guide is provided.

## [PowerRustCOBOL 1.20.0] — 2026-06-14

Documentation viewer with Markdown + Mermaid rendering.

### New

- **Help → Documentation.** A new window renders the embedded PowerRustCOBOL
  documentation (Markdown) with its **Mermaid** diagrams drawn inline — rendered
  in pure Rust (`mermaid-rs-renderer` → SVG → `resvg`), no Node/Chromium.
  - Two-pane layout: a searchable document list and a rendered viewer; the docs
    are embedded at build time (offline), and `Cmd+O` opens any local `.md`.
  - **File** (Print → PDF, Close), **View** (Zoom In/Out, Full Screen, Outline)
    and **Help** (Shortcuts) menus.
  - In-document **search** with **blue-on-yellow** match highlighting, a `Go`
    button and `Enter` to jump to the first match, `◀ / ▶` (and `,` / `.`) to
    step between matches with a live `n/total` counter; the focused match shows
    in orange and is scrolled into view.
  - A clickable **outline** (table of contents) **and** clickable in-document
    `[…](#…)` links that jump to their section.
  - An **icon toolbar** (vector icons) mirroring the shortcuts: open a file
    (Cmd+O), view source (Opt+Cmd+U), keep on top (Cmd+T), print (Cmd+P), close
    (Cmd+W).
  - Adjustable **font size** (`A+ / A−`, Cmd+`+` / Cmd+`-`) that is **remembered
    across sessions**; plus zoom, full-screen, and a view-source modal.
  - A translucent **frosted-glass** window (uneven procedural fog).
  - **Print** renders the document to a PDF (with the diagrams embedded) and
    opens it in the OS viewer. The PDF font is a system sans-serif extracted at
    runtime — nothing is bundled.
  - Theme-aware (adopts the IDE style) and I18N-aware (EN/ES/PT/JA/ZH).

## [PowerRustCOBOL 1.19.0] — 2026-06-14

Optional persistence for in-memory indexed files (`STORAGE IS MEMORY`).

### New

- **`STORAGE IS MEMORY WITH PERSISTENCE`** (SELECT-clause extension). An in-RAM
  indexed file can now opt into being written to its disk container **on `CLOSE`
  only** — never on `COMMIT`, so the in-memory performance profile is preserved.
  The phrase combines with compression (`STORAGE IS MEMORY WITH COMPRESSION WITH
  PERSISTENCE`).

### Changed

- **`STORAGE IS MEMORY` is now ephemeral by default.** Without `WITH
  PERSISTENCE`, a MEMORY file's contents are discarded at `CLOSE` (an existing
  disk file is still *loaded* on `OPEN`). `COMMIT`/`ROLLBACK` on a MEMORY file
  are pure in-RAM transaction boundaries and never touch disk.
- **`OPEN OUTPUT` always (re)creates the on-disk container** for a MEMORY file,
  regardless of the persistence setting, so the file exists on disk.
- The two published `STORAGE IS MEMORY` file-I/O tests were updated to declare
  `WITH PERSISTENCE` (they verify cross-`CLOSE` persistence). New self-checking
  test `tests/cobol/fileio/idx_mem_persist.cbl` covers both modes.

### Docs

- Developer's Guide §14: "Two storage modes" and "When data reaches disk"
  updated for the ephemeral default and `WITH PERSISTENCE`.

## [PowerRustCOBOL 1.18.0] — 2026-06-13

COBOL-85 language features: binary table search and file-error declaratives.

### New

- **`SEARCH ALL` (binary search).** `SEARCH ALL` now parses and executes as a
  true binary search over an `OCCURS` table declared with an
  `ASCENDING`/`DESCENDING KEY`. The `OCCURS … KEY IS …` phrase is captured
  (previously skipped) and drives the bisection; the `ALL` keyword is recognised
  after `SEARCH` regardless of token form. Serial `SEARCH` is unchanged.
- **`DECLARATIVES` / `USE AFTER STANDARD ERROR PROCEDURE`.** A
  `DECLARATIVES … END DECLARATIVES` block at the head of the `PROCEDURE DIVISION`
  registers file-error handlers. When a file verb (`OPEN`/`READ`/`WRITE`/
  `REWRITE`/`DELETE`/`START`/`CLOSE`) ends with an error `FILE STATUS` that the
  statement did not handle with its own `AT END` / `INVALID KEY` phrase, the
  matching `USE` procedure runs. Targets may be file names, an open mode
  (`INPUT`/`OUTPUT`/`I-O`/`EXTEND`), or a catch-all. New lexer tokens
  (`DECLARATIVES`, `USE`), AST (`ProcedureDivision.declaratives`,
  `UseProcedure`), parser, and runtime dispatch with a re-entrancy guard.

### Fixed

- **`NOT =` (and other negated relations) after `AND`/`OR`.** A negated relational
  condition on the right of a combined condition — e.g. `IF A NOT = X AND B NOT =
  Y` — now parses; previously the bare identifier before `NOT` was mis-read as an
  88-level condition-name, orphaning the `NOT`.
- **Arithmetic statement before a `NOT …` phrase.** An `ADD`/`SUBTRACT`/
  `MULTIPLY`/`DIVIDE`/`COMPUTE` used as the imperative of an `INVALID KEY` /
  `AT END` / `ON EXCEPTION` / `ON OVERFLOW` branch no longer swallows the
  following `NOT` (it previously mis-read `NOT INVALID KEY` etc. as the start of
  `NOT ON SIZE ERROR`). The `NOT` is now consumed only when it actually
  introduces `NOT [ON] SIZE ERROR`.
- **`CALL … USING` parameter passing (nested programs).** Arguments are now bound
  to the called program's `PROCEDURE DIVISION USING` LINKAGE items: values are
  copied in before the call and `BY REFERENCE` arguments receive the updated
  values on return (`BY CONTENT` / `BY VALUE` are not written back). Previously
  the arguments were ignored, so LINKAGE items stayed at their defaults.
- **`STRING … WITH POINTER`.** The pointer is now honoured: text is placed
  starting at the 1-based pointer position (preserving earlier bytes) and the
  pointer is advanced past the last byte moved, with overflow detected from that
  position. Previously the pointer was ignored.
- **Inline `PERFORM WITH TEST BEFORE/AFTER UNTIL`.** The inline (no-paragraph)
  form now accepts the optional `WITH` before `TEST` — e.g.
  `PERFORM WITH TEST AFTER UNTIL … END-PERFORM` — matching the out-of-line form.
  `TEST AFTER` runs the body once before evaluating the condition.
- **`EVALUATE` stacked `WHEN`.** Several consecutive `WHEN` phrases that share a
  single following imperative (e.g. `WHEN 1 WHEN 3 WHEN 5 MOVE …`) now all select
  that imperative, as COBOL-85 requires (previously the value-only `WHEN`s ran an
  empty branch).

### Docs

- Developer's Guide §13: new "Searching tables" and "Centralised file-error
  handling" subsections.

## [PowerRustCOBOL 1.17.0] — 2026-06-10

IDE visual redesign — "dark glass" look.

### Changed / New

- **Glass card panels.** The project tree, output, main pane and property
  inspector now sit on rounded, subtly-bordered glass cards with soft shadows
  (`theme::glass_panel_frame`).
- **Opaque, pane-matched background.** The whole window is painted with an opaque
  floor + the optional background image + the same pane fill, so the area around
  the panes matches the panes (no desktop bleed / no bright wallpaper in the
  gaps). The "Transparent background" option was **removed**.
- **Collapsible property section cards** in the form inspector (Form Properties /
  Target Device / Appearance / Background Image / Size / Events) with blue ▸/▾
  headers (`section_card`); the control inspector shares the same blue card-style
  section headers for consistency.
- **New "Deep Blue" theme** (17 total) — near-black glass panes with blue accents.
- **Full-width selection pill** + hover highlight in the tree; **left-aligned**,
  snug rows (fixes centred/jittery labels); grey indent/divider lines removed.
- **Solid semaphore knobs** — the green/yellow/red item-status dots are now crisp
  filled circles.
- **Standardised non-visual control icons** — Timer/AI-Agent/REST/SQL share one
  glass card and consistent stroke-drawn icons (no more mismatched colours, emoji
  tofu boxes or the one-off orange SQL cylinder).
- **Toolbar** reordered to **Open · Save · Check · Build · Run · Debug · Stop · ⚙**;
  the separate Debug row now only appears during an active debug session.
- **RAD properties panel** resizes up to half the window width (was capped at
  320px, clipping long values); **project tree** defaults to 410px wide.
- Roomier spacing, 8px control corners, larger fonts retained.

## [PowerRustCOBOL 1.16.0] — 2026-06-10

IDE: transparent-background option, calmer background, roomier UI.

### New features

- **Transparent background option** (Appearance dialog). When enabled, the IDE
  background colour is fully transparent — the desktop shows through the glass
  panels — and a background image, if set, **keeps its own transparency** (its
  alpha is preserved, scaled only by the opacity slider). Per project
  (`[ide] transparent_background`). In this mode the panels become more
  translucent so the desktop/image reads through.

### Changed

- **Calmer background, more readable panels.** With an opaque background the
  image is now drawn over the themed base and a **low-noise dark overlay** so it
  reads as a subtle backdrop instead of competing with the editor; panels stay
  at full readable opacity (they are no longer force-thinned just because an
  image is set).
- **Roomier, softer UI.** More spacing between rows and around sections (larger
  item spacing, button padding, row height, window/menu margins) and softer
  control corners (8 px radius) for a less cramped, more polished feel.

## [PowerRustCOBOL 1.15.2] — 2026-06-10

IDE: assets can be added and ship with the build.

### Fixed

- **The Assets category now accepts any file** (images, audio, video, fonts,
  data, …). The "Add" picker passed a `"*"` filter to the native dialog, which
  greyed out every file on macOS/GTK; assets now open with **no extension
  filter** so anything is selectable.
- **Adding a file from outside the project now imports it.** Previously a file
  outside the project directory was rejected ("must be inside the project
  directory"). The chosen file is now **copied into a category subfolder**
  (`src/`, `forms/`, `assets/`, `docs/`) and tracked, so it becomes part of the
  project. The add is also routed to the category you clicked (not guessed from
  the extension).

### Changed

- **Bundled assets ship with the native build.** `cobolt build` now copies every
  tracked Assets/Documentation file next to the produced binary (under `bin/`,
  preserving the project-relative layout), so images/audio/fonts are available
  to the program at runtime. (The `.zip` package already included them.)

## [PowerRustCOBOL 1.15.1] — 2026-06-10

IDE: background image now actually shows, lighter divider lines on dark themes,
and 10 more colour themes.

### Fixed

- **The IDE background image now appears.** It was painted on the background
  layer but the panels tiled the whole window at ~80–95 % opacity, hiding it.
  Now, when a background image is set, the panels become noticeably more
  translucent (frosted glass), the image is drawn over an **opaque themed base**
  (replacing the desktop bleed-through) so it reads as a real wallpaper, and the
  opacity slider dims it via a scrim. Default opacity raised to **70 %**.
- **Divider/border lines are light-grey on dark themes** (and a mid-grey on
  light themes) so separators are clearly visible against the dark chrome.

### New features

- **10 more colour themes** (16 total): Dracula, Nord, One Dark, Gruvbox Dark,
  Tokyo Night, Night Owl, Cobalt2, Solarized Light, GitHub Dark, and Material
  Palenight — alongside the existing Dark Glass (default), Dark+, Light+,
  Monokai, Solarized Dark and High Contrast.

## [PowerRustCOBOL 1.15.0] — 2026-06-10

IDE: selectable colour themes + per-project background image, and a real fix for
form edits not reflecting in the Main Pane.

### New features

- **IDE colour themes (VSCode-inspired).** A new **Appearance** dialog (the ⚙
  button on the toolbar) lets you pick a colour theme. Six themes ship:
  **Dark Glass** (the default — identical to the previous look), **Dark+**,
  **Light+**, **Monokai**, **Solarized Dark**, and **High Contrast**. The theme
  drives the whole IDE chrome *and* the COBOL editor's syntax colours. The choice
  is saved **per project** (`cobolt.toml` → `[ide] theme`). New `theme` module
  (`crate::theme`): a flat `Theme` palette + registry; `apply_glass_visuals` and
  the editor's syntax layouter both read it.
- **Per-project background image with opacity (transparency) control** — just like
  the RAD form designer. In the same Appearance dialog you can browse for an image
  and set its opacity (0–100 %); it is painted behind the translucent glass panels
  of the main IDE window, scaled to cover. Stored per project
  (`[ide] background_image` + `background_opacity`). `IdeSettings` added to the
  project model with serde defaults so existing projects upgrade transparently.

### Fixed

- **Form property changes now reflect in the Main Pane.** The inline
  form/control inspector loaded the form once and never refreshed, so edits made
  (and saved) in the Designer window — or any external write of the `.cfrm` —
  were not shown when you returned to the Main Pane. The inspector now
  **live-reloads from disk on modification-time change** (preserving the selected
  control), so saving a form anywhere is reflected immediately. (Regression test:
  `inspect_refresh_tests`.)

## [PowerRustCOBOL 1.14.0] — 2026-06-10

IDE: controlled project tree, read-only generated code, richer toolbar.

### New features

- **Controlled project treeview** with five fixed, IDE-owned top categories —
  **Forms · Common Code · Generated Code · Assets · Documentation** — each with a
  professional icon. The four developer categories have a `[+]` to add
  sub-entries; developers can only add files *within* a category, never create
  top nodes. (`Documentation` is a new category; `cobolt.toml` gains
  `documentation` + `generated` lists, loaded with serde defaults so existing
  projects upgrade transparently.)
- **The project itself is the tree root** (project name + version); the five
  categories nest under it. Category and file **icons are 80 % larger**, and
  everything **below level 3 is collapsed by default** (Project · Category · Item
  stay open).
- **Forms expand to their controls**, grouped by RAD toolbox category with
  **Non-Visual first** (then Common, Container, Data, Graphics, Menus, Charts,
  Dialogs). **Single-click a file** opens it in the **Main Pane** (formerly the
  editor area); **single-click a form** shows its properties inline, **double-click**
  opens the RAD designer.
- **Widget events in the tree.** A control with event handlers expands to an
  **Events** group; clicking an event opens the form's generated COBOL at that
  event's paragraph (read-only).
- **Selection highlight** — the clicked tree element is highlighted as selected.
- **Debug is gated on a Generated Code selection** — the Debug button is enabled
  only when a generated-code item is selected in the tree (debugging targets the
  RAD-generated backend), with an explanatory tooltip otherwise.
- **Inline property inspector in the Main Pane.** Clicking a form or one of its
  controls in the tree shows the **same properties pane as the RAD** in the Main
  Pane — edit parameters and they're saved back to the `.cfrm` without opening
  the designer (an "Open in Designer" button is offered for deeper edits). It
  **reuses the designer's `PropertiesPanel`** (and its `set_property`/
  `set_form_prop` logic) via a transient panel — no duplicated property code, no
  designer window.
- **Semaphore status dot** to the left of every tree element: **green** = tested/
  checked OK and unchanged, **yellow** = changed since the last check (or never
  tested), **red** = check found an error / failed. `do_check` sets green/red;
  editing a file (since its last check) flips it back to yellow; controls inherit
  their form's status.
- **Generated Code is its own read-only category.** Each form's RAD-generated
  COBOL (output of the form designer, one entry per form, named after it) lives
  under the **Generated Code** node — IDE-owned (no `[+]`), shown in blue with a
  🔒 badge, and opened **non-editable** in the editor (a flat-blue layout, never
  saved over) for review/debug only. Hand-written **Common Code** — the pure
  COBOL-85 modules `CALL`ed by forms — stays fully editable and contains no
  generated files.
- **Toolbar gains Build (binary), Run (interpreted) and Debug**, alongside Stop /
  Check / Open / Save.
- **Compile-gating**: Run / Debug / Build are enabled only when the project has
  at least one COBOL program (hand-written or generated) **or** at least one
  form; otherwise they're disabled with an explanatory tooltip.
- i18n: new keys for all five languages (categories, tree affordances, toolbar
  Build/Debug, the compile-gating tooltip).

### Design (not yet implemented)

- `docs/ide-collaboration-design.md` — the multi-developer collaboration design
  (Phase B): a **pluggable `SyncBackend`** (local-only · local git · GitHub ·
  Google Drive), pessimistic file-level locking (warn-once, read-only for the
  second developer, re-offer on release), change propagation, and a phased
  rollout starting from a trivial local backend. Design only — no code.

### Theme

- **Fonts are 50 % larger** (UI text styles and the code editor). The colour
  palette is unchanged (the dark glass theme is kept).

### Fixed

- **Form property changes now reflect in the IDE on save.** Saving a form (from
  the RAD designer or the inline Main-Pane inspector) refreshes the tree's cached
  form, **regenerates the backend COBOL** (so Generated Code reflects the change),
  keeps it tracked, and reloads any open generated editor tab.

### Tests

- `project_model` unit tests (category routing, generated detection incl. legacy
  stem-match, compile-gating). Full suite 414 passing.

## [PowerRustCOBOL 1.13.1] — 2026-06-10

Bug fix: `IF … ELSE …` sentence scoping (and `NEXT SENTENCE` with it).

### Fixed

- **A period-terminated `IF … ELSE …` (no `END-IF`) no longer absorbs the
  following sentences into the `ELSE` branch.** The parser now treats a period
  as a terminator of an `IF` branch, so subsequent sentences are siblings of the
  `IF`. This also fixes **`NEXT SENTENCE` inside an `IF … ELSE …`**, which had
  jumped one sentence too far (the statement after the IF was skipped). `NEXT
  SENTENCE` now lands correctly for both the period- and `END-IF`-terminated
  forms. (`crates/cobolt-parser/src/stmt.rs`: `parse_if`/`parse_stmts`.)

### Cleanup

- Removed dead `parse_recognized_noop` (its "UNLOCK/ALTER/RELEASE/RETURN no-op"
  comment was stale — all four are implemented). Renamed
  `parse_initialize_as_move` → `parse_initialize` and corrected its comment
  (INITIALIZE is fully implemented, not a MOVE-SPACES shortcut).

### Tests

- `test_control_flow`: NEXT SENTENCE in `IF … ELSE` (period and `END-IF`) and a
  plain `IF/ELSE` sentence-scoping regression. Full suite 410 passing.

## [PowerRustCOBOL 1.13.0] — 2026-06-10

INDEXED log rotation — keep each log file under 100 KiB.

### New feature

- **The INDEXED observability log now rotates** (logrotate/Grafana style). When
  the active `<assign-path>.log` approaches **100 KiB** it is renamed to
  **`<user|no-user>.<datafile>.log.<timestamp>`** and a fresh active log is
  started, so no single file grows without bound.
  - `<user>` is the `OPEN … WITH REGISTERED USER` value (sanitized for the
    filesystem); when the OPEN supplies no user, **`no-user`** is used in the
    rotated file name.
  - `<timestamp>` is a compact UTC stamp, e.g. `20260610T120230461Z`.
  - Rotated archives are complete, parseable logs; the runtime never deletes
    them (prune/ship them with your log pipeline).

### Tests & docs

- `indexed_log` unit tests for rotation (active stays under the cap; rotated file
  named with the user, and `no-user` when absent). Verified end-to-end via
  `rcrun` (a 700-commit run rotates at 512 lines, active stays ~38 KiB). Full
  suite 407 passing.
- `docs/observability.md` §1.2 documents rotation.

## [PowerRustCOBOL 1.12.0] — 2026-06-10

`OPEN … WITH REGISTERED USER` — record the operator in the INDEXED log.

### New language feature

- **`OPEN {INPUT|OUTPUT|I-O|EXTEND} file … WITH REGISTERED [USER] {literal |
  data-item}`** (PowerRustCOBOL extension). Since COBOL programs rarely sit
  behind an authentication engine, the operator/user is supplied explicitly on
  `OPEN`; it is recorded as a `user=` field on **every** event line of that
  file's session in the INDEXED observability log (`OPEN`/`COMMIT`/`ROLLBACK`/
  `CLOSE`). `USER` is optional; the value may be a string literal or a data item.
  Purely observational — no authentication/authorization, and no effect when the
  log is off.

### Docs & tests

- `docs/observability.md` §1.3.1 (the new clause + examples); the `user` field
  added to the field table. `docs/cobol85-supported-syntax.md` updated.
- Tests: parser (`open_with_registered_user_literal_and_data_item`) and an
  end-to-end interpreter+log assertion (`open_with_registered_user_appears_in_log`).
  Full suite 405 passing.

## [PowerRustCOBOL 1.11.0] — 2026-06-10

redb engine: read/write optimizations + an optional per-file transaction log.

### New features

- **Per-file INDEXED observability log** (redb engine). Enable with
  `rcrun --indexed-log <basic|full>` (`--indexed-log true` = `basic`) or
  `COBOL_INDEXED_LOG`. Each file gets a sidecar log at `<assign-path>.log`
  (e.g. `customers.idx` → `customers.idx.log`). One `key=value` line per
  transaction event (`OPEN`/`COMMIT`/`ROLLBACK`/`CLOSE`) records: ISO-8601 UTC
  timestamp, tx id, kind, write/rewrite/delete counts, records, bytes, duration,
  rec/s + bytes/s, and the **ordering quality** of the written keys
  (`order=ordered|unordered`, `in_order`/`out_of_order`). The `full` level also
  appends redb **index statistics** on `CLOSE` (tree height, leaf/branch/
  allocated pages, stored/fragmented bytes) — this walks the index, so it is
  opt-in. Logging is off by default and never affects program behavior.
- **Grafana/Loki-ready log formats.** `--indexed-log-format <text|json>`
  (`COBOL_INDEXED_LOG_FORMAT`) selects the line format. `text` is logfmt
  (Loki `| logfmt`); `json` emits **NDJSON** (Loki `| json`) with numeric metrics
  as bare JSON numbers so Grafana can graph them directly. Default `text`.

### Performance

- **READ NEXT** by the primary key of reference now returns the record straight
  from the range cursor (one B+tree descent per record instead of two) —
  ~17 µs/record sequential scan at 200 k.
- **WRITE** opens the `primary`/`alt` tables once per operation (was twice for
  the duplicate-check + insert). A micro-benchmark showed that caching the table
  handle *across* calls adds only ~8% over once-per-operation, so the simpler,
  `unsafe`-free single-open path was chosen; write cost is dominated by redb's
  ACID insert (~44 µs/record). Durability/crash-safety is unchanged.

### Docs & tests

- New `docs/observability.md` — the observability reference (starts with the
  INDEXED transaction log: flags, field table, formats, Grafana/Loki pipeline,
  cost/safety; plus `COBOLT_LOG` tracing and a roadmap).
- `docs/indexed-redb-engine.md` updated (optimizations; observability log now
  summarized with a pointer to `observability.md`).
- Tests: `indexed_log` unit tests (ISO timestamp, level parsing) and an
  end-to-end log assertion + sequential-scan timing in `test_indexed_redb.rs`.
  Full suite 400 passing.

## [PowerRustCOBOL 1.10.0] — 2026-06-05

Crash-safe INDEXED engine on a redb substrate (opt-in).

### New features

- **New `STORAGE IS DISK` engine for `ORGANIZATION IS INDEXED`**, built on
  **redb** (pure-Rust embedded ACID key-value store; copy-on-write B+tree, dual
  meta pages, per-page checksums). Opt-in via `--indexed-engine redb` or
  `COBOL_INDEXED_ENGINE=redb`; the default disk engine stays `PRCIDXD1`. It meets
  four operational goals the bespoke engine could not at scale:
  - **OPEN is O(1)** — only the meta page is read; no in-RAM record directory and
    no recovery scan, even after a crash (~5 ms to OPEN a 200 000-record file).
  - **RANDOM/NEXT reads** are B+tree / range operations over redb's page cache
    (~21 µs per random read at 200 000 records).
  - **Resident RAM = working set**, not record count (≥250 M records).
  - **Crash safety** — `COMMIT` is a durable redb transaction commit, `ROLLBACK`
    is an abort; a power loss can never leave a torn index.
- Behavioral parity with the default engine: the same versioned fixtures
  (`idx_crud` / `idx_persist` / `idx_tx`) run identically under redb (CRUD,
  primary + alternate `WITH DUPLICATES` in creation order, persistence,
  `COMMIT`/`ROLLBACK`), with matching file-status codes.
- Pure-Rust dependency (`redb`), no system library — consistent with the bundled
  SQLite / rustls philosophy.

### Docs & tests

- New guide: `docs/indexed-redb-engine.md` (goals, table layout, transaction
  model, parity, limits). Cross-referenced from `docs/indexed-file-internals.md`.
- Tests: `test_indexed_redb.rs` — the three fixtures under redb + direct
  `IndexedStore` checks + an `#[ignore]`d scale smoke test. Full suite 397 passing.

### Notes

- Bulk `WRITE` throughput (~20 k rec/s in one transaction) is a one-time load
  cost; OPEN, reads, and crash-safety are unaffected. Faster bulk loading is a
  tracked future optimization. Promoting redb to the disk default is deferred
  until it has more mileage.

## [PowerRustCOBOL 1.9.0] — 2026-06-05

PostgreSQL and MySQL support for the database runtime.

### New features

- **The SQL database runtime now speaks three backends** — SQLite,
  **PostgreSQL**, and **MySQL** — behind one unchanged CALL surface
  (`COBOL-OPEN-DB` / `COBOL-EXEC-SQL` / `COBOL-FETCH-ROW` / `COBOL-NEXT-ROW` /
  `COBOL-ROW-COUNT` / `COBOL-CLOSE-DB`). The engine is selected from the
  connection string's scheme:
  - `:memory:` / `sqlite:<path>` / bare path → **SQLite** (bundled)
  - `postgres://…` / `postgresql://…` → **PostgreSQL** (`postgres`, sync)
  - `mysql://…` → **MySQL** (`mysql`, rustls)
  - A COBOL program is portable across all three — only the connection string
    literal changes.
- All values are normalised to text uniformly across backends (NULL → spaces,
  integers/reals as digits, dates as `YYYY-MM-DD[ HH:MM:SS]`), so existing
  `COBOL-FETCH-ROW` code is unaffected.
- **Pure-Rust drivers** — both new backends build with no system library
  (`libpq`/`libmysqlclient`) and no OpenSSL; MySQL uses rustls.
- Form-designer **SqlDatabase** control: the `Driver` property now labels
  generated comments as SQLite / PostgreSQL / MySQL (routing stays by
  connection string).

### Docs & tests

- New guide: `docs/database-runtime.md` (connection strings, CALL reference,
  value normalisation, transactions, TLS notes, testing).
- Tests: connection-string routing + value normalisation + in-memory SQLite CRUD
  (`db_runtime` unit tests, `test_sql.rs`), plus opt-in `#[ignore]`d live
  PostgreSQL/MySQL round-trips (`PRC_TEST_PG_URL` / `PRC_TEST_MYSQL_URL`).

### Notes

- The synchronous PostgreSQL driver connects without TLS (`NoTls`); see
  `docs/database-runtime.md` for the recommended TLS approach. The COBOL
  `COMMIT`/`ROLLBACK` verbs remain INDEXED-file transactions — use
  `COBOL-EXEC-SQL` with `BEGIN`/`COMMIT`/`ROLLBACK` for SQL.

## [PowerRustCOBOL 1.8.0] — 2026-06-05

Program-controlled `COMMIT` / `ROLLBACK` transactions for INDEXED files.

### New language features

- **`COMMIT` and `ROLLBACK`** are now real COBOL verbs (reserved keyword tokens,
  so a preceding `DISPLAY` no longer absorbs them). They apply to **every** open
  INDEXED file in the run unit:
  - `OPEN` begins a transaction; `COMMIT` makes all changes durable and starts a
    new one; `ROLLBACK` undoes every `WRITE`/`REWRITE`/`DELETE` since the last
    `COMMIT`/`OPEN`; `CLOSE` persists (implicit commit).
  - The **memory engine**'s existing journal is now wired through.
  - The **disk engine** gained a real in-run **undo log** (Insert/Update/Delete
    inverses) — `ROLLBACK` was previously a no-op there.

### Notes

- This is *program-level* rollback; crash-recovery via a durable write-ahead log
  remains future work.
- New tests: `test_transactions` (disk + memory engines). Full suite: **382
  passed, 0 failed**.

## [PowerRustCOBOL 1.7.2] — 2026-06-05

File-sharing / locking phrases and `CANCEL` — previously parse errors or no-ops.

### New language features

- **`OPEN … [SHARING WITH {ALL OTHER | NO OTHER | READ ONLY}] [WITH LOCK]`** —
  parses and is honoured where meaningful (advisory in the single-run-unit model;
  no longer a parse error).
- **`READ … WITH [NO] LOCK` / `WITH KEPT LOCK`** — `WITH NO LOCK` releases the
  record lock the INDEXED engine takes under `I-O`.
- **`UNLOCK file [RECORD[S]]`** now releases the file's INDEXED record locks
  (new `IndexedStore::unlock`).
- **`CANCEL program …`** — was silently dropped at parse; now a real statement
  that re-initialises the named (nested) program's WORKING-STORAGE so the next
  `CALL` starts fresh.

### Notes

- New tests: `test_file_locking` (lock flow + CANCEL) and parser cases in
  `test_statements`. Full suite: **378 passed, 0 failed**.

## [PowerRustCOBOL 1.7.1] — 2026-06-05

Completes the previously recognized-but-no-op `ACCEPT` register sources.

### New language features

- **`ACCEPT … FROM COMMAND-LINE`** — the whole command line (arguments joined).
- **`ACCEPT … FROM ARGUMENT-NUMBER`** — the count of command-line arguments;
  **`DISPLAY n UPON ARGUMENT-NUMBER`** sets the argument pointer, and
  **`ACCEPT … FROM ARGUMENT-VALUE`** returns the argument at that pointer.
- **`ACCEPT … FROM ENVIRONMENT-VALUE`** — the value of the variable named by
  **`DISPLAY "name" UPON ENVIRONMENT-NAME`** (paired registers).
- **`ACCEPT … FROM ESCAPE KEY`** → `"00"`, **`FROM CRT STATUS`** → `"0000"`.
- The CLI passes a program's own arguments through (`rcrun run prog.cbl a b c`),
  and a compiled binary uses its real `argv`.

### Notes

- New test: `test_accept_sources`. Full suite: **373 passed, 0 failed**.

## [PowerRustCOBOL 1.7.0] — 2026-06-04

Avoid-list clearance: the remaining ⚠️/❌ items in the RustCOBOL-85 Supported
Syntax Reference are now implemented. The COBOL-85 verb/clause set is fully
covered. The IDE is unchanged.

### New language features

- **Identifier-object abbreviated conditions** — `a = b OR c` (where `c` is a
  data item) is resolved at runtime via the 88-level metadata (new
  `Condition::NameOrAbbrev`): a known condition-name evaluates as one, otherwise
  it is the abbreviation object `a = c`.
- **`INITIALIZE … REPLACING {ALPHABETIC|ALPHANUMERIC|NUMERIC|…-EDITED} [DATA] BY
  value`** — sets each subordinate item of that category; others untouched.
- **`66 RENAMES item-1 [THRU item-2]`** — a regrouping alias; reads synthesize
  the concatenated value, writes distribute by field width.
- **Pointers** — `USAGE POINTER`; `SET ptr TO {ADDRESS OF id | NULL | ptr2}`;
  `SET ADDRESS OF id TO {ptr | ADDRESS OF x | NULL}` (aliases `id` onto the
  target's storage — reads **and** writes follow it); `IF ptr = NULL`.
- **`ALTER para-1 TO [PROCEED TO] para-2`** redirects para-1's `GO TO`;
  **`UNLOCK file`** is a real statement (no-op in the auto-unlock model).
- **Faithful `NEXT SENTENCE`** — was never actually parsed; now recognized and
  it transfers control past the next sentence boundary (synthetic markers).
- **Remaining standard intrinsics** — `PRESENT-VALUE` (completes the COBOL-85
  set) plus `YEAR-TO-YYYY`, `BYTE-LENGTH`/`LENGTH-AN`, `NUMVAL-F`, `TEST-NUMVAL`.
- **Extended screen `ACCEPT`/`DISPLAY`** — `DISPLAY … AT {nnnn | LINE n COLUMN n}
  [WITH HIGHLIGHT|REVERSE-VIDEO|UNDERLINE]` and `ACCEPT … AT …` execute via ANSI
  cursor positioning + SGR in CLI mode (ignored in GUI mode — the designer
  supersedes SCREEN I/O there).

### Notes

- New tests: `test_pointers`, plus cases in `test_conditions`, `test_initialize`,
  `test_control_flow`, `test_intrinsics_date`, and `test_statements`. Full suite:
  **371 passed, 0 failed**.

## [PowerRustCOBOL 1.6.0] — 2026-06-04

A COBOL-85 verb-completeness pass: closing every remaining ⚠️/❌ item in the
RustCOBOL-85 Supported Syntax Reference. The IDE is unchanged.

### New language features

- **Multi-receiver `MULTIPLY`/`DIVIDE GIVING` + per-receiver `ROUNDED`** —
  `MULTIPLY a BY b GIVING r1 [ROUNDED] r2 …`, `DIVIDE … GIVING q1 [ROUNDED] q2 …
  [REMAINDER r]`, and per-receiver `ROUNDED` on `ADD`/`SUBTRACT`. (Also fixes
  `MULTIPLY a BY b` with no GIVING to store into `b`.)
- **`EXIT PERFORM [CYCLE]` / `EXIT PARAGRAPH` / `EXIT SECTION`** via control-flow
  signals; plain `EXIT` is now a no-op return point and `EXIT PROGRAM` returns to
  the caller (both were wrongly `STOP RUN`).
- **`CALL … NOT ON EXCEPTION`** — the body now runs when the call resolves.
- **`INSPECT … TALLYING … REPLACING`** combined (the REPLACING half was dropped)
  and **`BEFORE/AFTER INITIAL`** region qualifiers on every TALLYING/REPLACING
  phrase; TALLYING now accumulates onto its counter.
- **Date / financial intrinsics** — `INTEGER-OF-DATE`, `DATE-OF-INTEGER`,
  `INTEGER-OF-DAY`, `DAY-OF-INTEGER`, `FRACTION-PART`, `ANNUITY` (were `0`).
- **Literal-object abbreviated conditions** — `A = 1 OR 2 OR 3` reuses the
  subject and operator.
- **`EVALUATE … ALSO`** multi-subject (positional AND matching) and **`WHEN NOT`**.
- **Real 88-level condition-names** — the host item is tested against the
  declared VALUEs/ranges, and `SET 88-name TO TRUE/FALSE` writes a satisfying /
  violating value to the host (previously a bogus standalone slot).
- **`PERFORM para VARYING …`** now executes the named paragraph each iteration.
- **Functional `SORT` / `MERGE`** — `RELEASE`/`RETURN`, `USING`/`GIVING`, and
  `INPUT`/`OUTPUT PROCEDURE`, with stable sort by ASCENDING/DESCENDING keys.

### Notes

- `UNLOCK` and `ALTER` remain recognized no-ops (correct for the auto-unlock
  model; ALTER is deprecated). `66 RENAMES`, `INITIALIZE … REPLACING`, and
  identifier-object abbreviation remain unsupported (documented in the reference).
- New tests: `test_arith_receivers`, `test_control_flow`, `test_inspect`,
  `test_intrinsics_date`, `test_conditions`, `test_sort` (cobolt-runtime).

## [PowerRustCOBOL 1.5.0] — 2026-06-04

Hierarchical / occurrence-aware runtime environment. One dedicated effort
unblocks four interrelated COBOL-85 capabilities that the flat data store
previously could not express. The IDE is unchanged.

### New language features

- **Runtime table subscripting** — `TABLE-ITEM(i)` (and multi-dimension
  `T(i, j)`) now read and write per-occurrence storage slots, materialised
  lazily from the item's template on first write. Variable subscripts
  (`T(WS-I)`) are evaluated each access.
- **Qualified-name disambiguation** — `data-item OF group` / `… IN group`
  now resolves to the correct item when a leaf name is **declared in more than
  one group**. Duplicated names are stored under path-qualified canonical keys,
  so `BALANCE OF ACCOUNT` and `BALANCE OF SUMMARY` are independent fields
  (previously they collided into one slot). Unique names are unaffected.
- **`MOVE CORRESPONDING g1 TO g2`** — moves each subordinate item that the two
  groups share by name, recursing through matching sub-groups; items present in
  only one group are untouched.
- **`ADD CORRESPONDING g1 TO g2 [ROUNDED]`** and
  **`SUBTRACT CORRESPONDING g1 FROM g2 [ROUNDED]`** — new
  `Stmt::AddCorresponding` / `Stmt::SubtractCorresponding`; combine each matching
  numeric pair, recursing through matching sub-groups.
- **Functional `SEARCH` / `SEARCH ALL`** — `Stmt::Search` now drives the table's
  index (the `VARYING` item, else its first `INDEXED BY` index) from its current
  value to the table bound, evaluating each `WHEN` per occurrence and running the
  first matching imperative, else the `AT END` body. `INDEXED BY` index-names are
  registered as numeric index registers (recognised by `SET` and the resolver).
- **`DISPLAY` of qualified & subscripted numerics** now renders with full PIC
  width (leading zeros), matching plain-item DISPLAY.

### Internal

- `CobolEnvironment` gains a per-item symbol table (`ItemSym`: OCCURS dims, child
  names + canonical child keys, ancestor path, INDEXED BY names) plus a
  duplicate-name index; `resolve_name()` maps a (name, qualifiers) reference to
  its canonical storage key.
- Tests: `crates/cobolt-runtime/tests/test_hierarchy.rs`.

## [PowerRustCOBOL 1.4.0] — 2026-06-04

A COBOL-85 language-coverage pass: closing parser/runtime gaps surfaced by the
verb test matrix. The IDE is unchanged.

### New language features

- **Reference modification** `data-item(start:[length])` — new `Expr::RefMod`,
  parsed on any operand (disambiguated from subscripts by the `:`), evaluated as
  a substring (sender) and as a spliced partial assignment (receiver).
- **`COMPUTE` multiple receivers + per-receiver `ROUNDED`** —
  `COMPUTE r1 [ROUNDED] r2 [ROUNDED] … = expr` (was single receiver, one flag).
- **Category-aware `INITIALIZE`** — new `Stmt::Initialize`; numeric / numeric-
  edited items reset to ZERO, everything else to SPACES, recursing into groups
  (was a blanket `MOVE SPACES`).
- **`STRING` / `UNSTRING … ON OVERFLOW` / `NOT ON OVERFLOW`** + the
  `END-STRING` / `END-UNSTRING` / `END-SEARCH` scope-terminator tokens (which also
  fixes `DISPLAY` greedily swallowing a following `END-*` word).
- **`SET idx {UP|DOWN} BY n`** (encoded as ADD / SUBTRACT).
- **Inline `PERFORM n TIMES … END-PERFORM`** (no paragraph).
- **Operator-prefixed abbreviated conditions** — `a > 1 AND < 9`, `a = 5 OR = 7`.
- **`CALL … ON EXCEPTION / ON OVERFLOW`** — the handler now runs when the called
  program is unresolved (was parsed and discarded).
- **Extended `ACCEPT` / `DISPLAY` screen forms recognized** — `AT nnnn`,
  `AT LINE n COLUMN n`, `WITH <attributes>`, and `ACCEPT FROM
  {ARGUMENT-NUMBER|ARGUMENT-VALUE|ENVIRONMENT-VALUE|ESCAPE KEY|CRT STATUS}` parse
  (not executed — SCREEN I/O is superseded by the designer).
- **`SEARCH` / `SEARCH ALL`, `RELEASE`, `RETURN`, `UNLOCK`, `ALTER`** are now
  recognized statements (parse as no-ops) instead of breaking the parse.
- **Intrinsic functions** expanded: `ORD`, `CHAR`, `ORD-MAX`, `ORD-MIN`, `SUM`,
  `MEAN`, `MEDIAN`, `MIDRANGE`, `RANGE`, `VARIANCE`, `STANDARD-DEVIATION`,
  `FACTORIAL`, `SIN`/`COS`/`TAN`/`ASIN`/`ACOS`/`ATAN`, `LOG`/`LOG10`,
  `EXP`/`EXP10`, `PI`, `STORED-CHAR-LENGTH`, `WHEN-COMPILED` (was: unknown
  functions returned 0).

### Known gaps (documented)

- `MOVE/ADD/SUBTRACT CORRESPONDING`, runtime **table subscript indexing**,
  **qualified-name disambiguation**, and **functional `SEARCH`** all await an
  occurrence-aware data model (the runtime store is currently flat).
- Multiple receivers on `MULTIPLY`/`DIVIDE`; per-receiver `ROUNDED` on
  `ADD`/`SUBTRACT`; `SET ADDRESS OF`; identifier-object abbreviated conditions.

### Docs

- New [`docs/cobol85-verb-test-matrix.md`](docs/cobol85-verb-test-matrix.md)
  (what to test) and [`docs/cobol85-supported-syntax.md`](docs/cobol85-supported-syntax.md)
  (the exact grammar RustCOBOL accepts, with an avoid-list). README updated.

## [PowerRustCOBOL 1.3.1] — 2026-06-04

File I/O fixes surfaced by the storage/compression File I/O test pack
(`tests/cobol/fileio/`), now run end-to-end in the suite.

### Fixes

- **Record `ORGANIZATION IS SEQUENTIAL` READ** — fixed-length records (no
  terminator) are now read one record (`record_len` bytes) per `READ`, dispatched
  by organization. Previously the reader used line reads for every sequential
  file, so the first `READ` of a record-sequential file consumed the whole file
  and subsequent reads hit EOF. (`interpreter.rs`)
- **Source is always free form.** `rcrun` no longer auto-detects fixed vs free;
  it treats source as free form (set `COBOLT_FIXED=1` to opt into fixed-form
  parsing). This keeps long `ASSIGN` paths / `DISPLAY` literals from being
  truncated at column 72.

### Grammar (final, lean)

- The INDEXED storage clause is **`STORAGE [MODE] IS MEMORY | DISK`** (`MODE`
  optional) and compression is **`WITH COMPRESSION`** — in the storage clause or
  as a standalone clause (which uses the default storage backend). The earlier
  `WITH COMPRESSION` spelling and other variations were removed to keep the
  grammar clean.

### Behaviour

- **Default storage is `DISK`.** When an INDEXED file has no `STORAGE` clause,
  it now uses the on-disk paged B+tree engine (was MEMORY). `STORAGE IS MEMORY`
  selects the in-RAM engine explicitly.
- Writing a record that creates a duplicate value on an `ALTERNATE RECORD KEY …
  WITH DUPLICATES` is now a fully successful `00` write (previously the
  informational `02`). `WITHOUT DUPLICATES` violations still return `22`.

### Tests

- The File I/O test pack is vendored under `tests/cobol/fileio/` (baseline
  `fileiot.cbl` + six storage/compression variants) and driven end-to-end by
  `crates/cobolt-runtime/tests/test_fileio_storage.rs` (ASSIGN paths redirected
  to a temp dir; the 1,000,000-record profile loop shrunk for speed — the
  original files keep the full 1M profile for manual `rcrun` benchmarking).
- The earlier `tests/cobol/indexed-files/` programs (idxbasic, idxstorage) were
  removed — the File I/O suite supersedes them with broader indexed coverage.
  Focused inline engine checks remain in `test_indexed.rs`.

## [PowerRustCOBOL 1.3.0] — 2026-06-04

INDEXED files gain a selectable storage backend and record compression.

### `STORAGE IS MEMORY | DISK` (new) + persistent on-disk B+tree

- **New SELECT clause** `STORAGE IS MEMORY | DISK [WITH COMPRESSION]`
  for INDEXED files (a PowerRustCOBOL extension). `ASSIGN TO` is still required —
  it is where the data is persisted. Parsed in `parse_file_control_entry`
  (`StorageMode` on `FileControl`); the parser now also recognises the spaced
  `ALTERNATE RECORD KEY … [WITH DUPLICATES]` form.
- **`MEMORY`** (default) — the existing in-RAM `BTreeMap` engine (whole file in
  memory, persisted to the `PRCIDX1` container on close).
- **`DISK`** — a new **persistent, paged on-disk B+tree engine**
  (`cobolt-runtime/src/indexed_disk.rs`, container `PRCIDXD1`): records and
  indexes live in the `ASSIGN` file and are read on demand, so RAM use is bounded
  by the page cache rather than the whole data set. Built from 4 KiB pages with
  a **free list** (freed pages reused), one **B+tree per key** (primary +
  alternates; variable byte-packed nodes, split on insert, doubly-linked leaves
  for `START` + `READ NEXT/PREVIOUS`), a **RecordId directory** (a record that
  moves on `REWRITE` only updates the directory, not every index), and **slotted
  data pages** with an overflow chain for oversized records. The full COBOL verb
  set works on it (`OPEN`/`WRITE`/`READ` random+sequential/`REWRITE`/`DELETE`/
  `START` with all key relations, `INVALID KEY`), with FILE STATUS 22/23/35/39.
  Index deletes are lazy (no node merge; data pages are reclaimed).
- Both backends share one `IndexedStore` trait, dispatched from
  `make_indexed_engine` by `STORAGE MODE`.

### `WITH COMPRESSION` (new)

- Optional `WITH COMPRESSION` compresses stored record data in **both**
  storage modes via a self-contained, **dependency-free** PackBits-style RLE
  (`cobolt-runtime/src/compress.rs`) chosen for maximum speed; a one-byte tag
  guarantees the output never grows. On the padded, fixed-length records typical
  of COBOL it compresses well past the 50 % target; incompressible blocks fall
  back to raw.

### Tests

- `compress.rs` (round-trip, ≥50 % on padded records, raw fallback, long runs),
  `indexed_disk.rs` (pager/free-list, B+tree splits over 2 000 records +
  persistence, all `START` relations, NEXT/PREVIOUS, alt keys with/without
  duplicates, REWRITE/DELETE, compression round-trip, status 35/39), and
  end-to-end COBOL `STORAGE IS DISK [WITH COMPRESSION]` programs in
  `tests/test_indexed.rs`.

## [PowerRustCOBOL 1.2.0] — 2026-06-03

A COBOL-85 language milestone: exact numeric arithmetic, numeric-edited
PICTUREs, `COPY`/`REPLACE` copybooks, and a full **INDEXED (ISAM) file engine**.
The IDE interface is unchanged; all generated COBOL source stays in English.

### Indexed (ISAM) files — new

- **Built-in keyed-file engine** (`cobolt-runtime/src/indexed.rs`) — a
  dependency-free ISAM store: primary `RECORD KEY` plus
  `ALTERNATE RECORD KEY [WITH DUPLICATES]`, records held in ascending key order,
  a journaled write log with `COMMIT` / `ROLLBACK`, and record locking. No
  external libraries.
- **Self-describing `PRCIDX1` container** — the on-disk format now embeds the
  full file schema (record format + every key's byte-ranged composite parts,
  encoding, ordering, duplicate policy, and COBOL field name) plus timestamps
  and a CRC-32 trailer, modelled on Fujitsu's `cobfa_indexinfo()` metadata so a
  future Fujitsu importer can write faithful files. The legacy records-only
  `PRCISAM1` container is still read (and upgraded to `PRCIDX1` on next write).
  - **Discovery API** `IndexedFile::inspect_path()` reads a file's schema
    (`IndexedFileInfo`) without opening it for I/O.
  - **Strict open-time validation**: declared `SELECT`/`FD` keys + record format
    are checked against the stored schema → FILE STATUS **39** on mismatch;
    `OPEN INPUT` of a missing file → **35**; corrupt container (CRC) → **90**.
  - Format documented in [`docs/indexed-file-format.md`](docs/indexed-file-format.md).
- **Verbs dispatched by `ORGANIZATION`.** `OPEN` / `CLOSE` / `READ` / `WRITE`
  are wired to each file's declared organization (from its `SELECT`), not a
  single hard-coded type, so SEQUENTIAL / LINE SEQUENTIAL / INDEXED share the
  common verbs while each keeps its own semantics. (`interpreter.rs`,
  `cobolt-runtime/src/files.rs` `RecordLayout` materialize/distribute.)
- **Indexed verb set executes**: `OPEN INPUT/OUTPUT/I-O/EXTEND`,
  `WRITE`, random `READ` by `RECORD KEY`, `READ … NEXT / PREVIOUS`
  (sequential), `REWRITE`, `DELETE`, and `START … KEY IS = / > / >= / < / <=`
  (incl. `GREATER/LESS THAN`, `NOT LESS THAN`).
- **`ACCESS MODE SEQUENTIAL / RANDOM / DYNAMIC`** now all execute (an
  unqualified `READ` is random under RANDOM/DYNAMIC; `NEXT/PREVIOUS` force
  sequential).
- **`INVALID KEY` / `NOT INVALID KEY`** phrases added to `READ`/`WRITE`/
  `REWRITE`/`DELETE`/`START`, alongside full **FILE STATUS** codes
  (00/02/10/22/23/…).
- **Selectable engine** — `rcrun --indexed-engine <rust|rm-cobol85|fujitsu>`
  (or `-I`) and the `COBOL_INDEXED_ENGINE` environment variable choose the ISAM
  engine. All engines are behaviour-compatible; `rust` is the default and
  `rm-cobol85` / `fujitsu` currently delegate to it pending their native
  container formats.
- Verified by the File I/O suite [`tests/cobol/fileio/`](tests/cobol/fileio/)
  plus `cobolt-runtime` integration and unit tests.

### Exact numeric arithmetic

- `ADD` / `SUBTRACT` / `MULTIPLY` / `DIVIDE` / `COMPUTE` run on an `i128`
  fixed-point mantissa (no `f64` round-trips): exact to 18-digit standard and
  31-digit extended precision, with `ROUNDED` (half away from zero) and
  `ON SIZE ERROR` / `NOT ON SIZE ERROR`. Decimal literals are carried exactly
  from the lexer. Numeric `DISPLAY` renders at full PIC width.
  Verified by [`tests/cobol/numeric-precision/numprec.cbl`](tests/cobol/numeric-precision/numprec.cbl).

### Numeric-edited PICTUREs

- Edit engine (`cobolt-runtime/src/numedit.rs`): `Z` suppression, `*`
  check-protection, fixed/floating `$` and `+`/`-`, `,`/`.` insertion,
  `B`/`0`/`/` insertion, and `CR`/`DB`, applied on `MOVE`/`DISPLAY` into an
  edited field.
- **`DECIMAL-POINT IS COMMA`** — comma decimal separator for literals and the
  swapped `.`/`,` roles in edited PICs.
  Verified by [`tests/cobol/numeric-edited-pic/`](tests/cobol/numeric-edited-pic/).

### COPY / REPLACE copybooks

- Preprocessor (`cobolt-runtime/src/copybook.rs`) expands
  `COPY name [OF lib] [REPLACING ==a== BY ==b== …]` (pseudo-text + word
  replacement), resolves copybooks beside the source, expands nested `COPY`
  recursively, and applies `REPLACE … BY …` / `REPLACE OFF`.
  Verified by [`tests/cobol/copy-replace/`](tests/cobol/copy-replace/).

### Tests

- `tests/cobol/` reorganized into per-purpose subfolders
  (`numeric-precision/`, `numeric-edited-pic/`, `copy-replace/`,
  `indexed-files/`).

## [PowerRustCOBOL 1.1.0] — 2026-06-01

### Form Designer & rendering

- **New control: Animator.** Plays animated images — **GIF, WebP and APNG** (and
  any still image) — decoded natively via the `image` crate (no external/FFmpeg
  dependency). Properties: `Source`, `AutoPlay`, `Loop`, `SizeMode`
  (Fit/Fill/Stretch/Center), back/border. Decoding + frame-timed egui playback
  live in the new shared `cobolt-media` crate, so the control animates in the
  designer canvas, the preview, the run-form **and** the compiled standalone
  binary. (MP4 support is planned via a native decoder behind the same API.)


- **System font picker** — the Font property is now a dropdown of the fonts
  installed on the machine (via `fontdb`), each name rendered **in its own
  font**. The list is virtualised, so only the families you actually scroll
  past are loaded. The chosen font **family and size** are now applied to the
  rendered text in the **designer canvas, preview window and run form**, with a
  graceful fallback to the built-in (Arial-like) proportional font when a family
  is Arial/default or unavailable on the target system. Bitmap-only faces (e.g.
  `GB18030 Bitmap`) that egui can't rasterise are rejected up-front, fixing a
  crash when scrolling the font list. (`cobolt-ide/src/fonts.rs`)

- **#69 — Resize the form canvas by dragging its border.** Right, bottom and
  bottom-right corner grips; live resize with grid snap and a minimum size.
  (`designer.rs`)

- **#70 — Double-click an event row to jump to its COBOL paragraph.** The
  generated `.cbl` is opened in the editor and scrolled to the paragraph (or
  `PROGRAM-ID`) definition. Single-click still opens the per-event modal editor.
  (`properties.rs`, `app.rs`, `editor.rs`; i18n key `hint_dblclick_event`)

- **#129 — Preview animations now apply `scale`.** Zoom/spin/flip animations
  resize controls in the preview window, via the shared
  `designer::scale_rect_about_center()` (also used by the canvas). (`app.rs`)

### Runtime / language

- **COBOL sequential file I/O — `ORGANIZATION IS SEQUENTIAL` and
  `LINE SEQUENTIAL`.** The ENVIRONMENT DIVISION's `FILE-CONTROL` is now parsed
  (`SELECT … ASSIGN TO … ORGANIZATION IS [LINE] SEQUENTIAL [ACCESS MODE …]
  [FILE STATUS IS …]`), and the runtime implements `OPEN INPUT/OUTPUT/EXTEND/I-O`,
  `WRITE record [FROM …]`, `READ file [INTO …] [AT END …] [NOT AT END …]`, and
  `CLOSE`, updating the FILE STATUS item (00/10/30/35/…). LINE SEQUENTIAL writes
  newline-terminated records (trailing spaces dropped); record SEQUENTIAL writes
  fixed-length records. `ASSIGN TO` accepts a literal path or a data item holding
  the path. `READ … AT END` accepts the two-word `AT END` / `NOT AT END` forms.
  (`cobolt-ast`, `cobolt-parser`, `cobolt-runtime`)

- **New built-in CALLs `COBOL-APPEND-FILE` / `COBOL-WRITE-FILE`** —
  `USING path text [status]` append a line to (or truncate+write) a text file.
  COBOL `OPEN/WRITE` file I/O is still unimplemented; these cover the common
  "write a results/log file" need. (`interpreter.rs`)

- **PICTURE repetition counts are now honored.** `analyze_pic` ignored `(n)`, so
  `PIC X(20)` held 1 char and `PIC 9(5)` had 1 digit. Templates are now expanded
  (`X(20)`→20, `9(7)V99`→7.2), and `PicClause.digits/decimals` widened to `u16`
  so wide fields like `PIC X(4096)` / `PIC X(32767)` are exact. (`cobolt-parser`,
  `cobolt-ast`)

- **Alphanumeric comparison pads with spaces.** `compare_values` compared raw
  strings, so a space-padded `PIC X(64)` field never equalled a short literal
  (e.g. `EVALUATE control-id WHEN "BTN-OK"` never matched). The shorter operand
  is now space-padded per COBOL rules. (`interpreter.rs`)

- **`STRING … DELIMITED BY SIZE` works.** The bare word `SIZE` lexes to the
  `SizeError` token (reserved for ON SIZE ERROR); the STRING parser now accepts
  it as the SIZE delimiter, so `STRING` no longer dropped all operands.
  (`cobolt-parser`)

### Compiler (standalone binary)

- **Richer Label rendering in the generated form app.** The compiled binary's
  Label now honors BackColor, ForeColor, FontSize, Bold/Italic/Underline/
  Strikethrough, TextAlign, WordWrap, Padding, Opacity, BorderStyle/BorderColor,
  Cursor (on hover), per-control geometry overrides (`X/Y/Width/Height`) and
  `Dock` from `COBOL-SET-PROPERTY`, plus a short input warm-up so a click already
  underway as the window opens can't trigger a control. (`cobolt-compiler`)

### Fixes

- Fixed a long-broken `cobolt-codegen` test target (ambiguous `.into()` in
  `Control::new` calls) and corrected stale form-event paragraph-name
  expectations (`MAIN-FORM--ONLOAD`, not `--ON-LOAD`).

- **Lexer — fixed-form identification area now stripped.** `flatten_fixed` /
  `preprocess_fixed` were slicing active source out to char-column 255 instead
  of 72, so anything a program placed in columns 73–80 (the identification area)
  leaked into the token stream. Now correctly cut at column 72. (`source.rs`)

- **Lexer — `END-PERFORM` is a scope-terminator keyword.** Corrected stale tests
  that asserted it should be an identifier; the keyword table and parser have
  always treated it as `Token::EndPerform` (like `END-IF` / `END-EVALUATE`).

- **Parser — sequential program units in one file are no longer dropped.**
  `parse_program` now collects sibling program units that follow the first
  program's `END PROGRAM` terminator (e.g. `OUTER. … END PROGRAM OUTER.` then
  `SET-RESULT. … END PROGRAM SET-RESULT.`) into `nested_programs`, so the runtime
  can `CALL` them. True nesting (inner units before the outer terminator, the
  codegen shape) is unchanged. Fixes all 6 `cobolt-runtime` nested-program tests.
  New regression tests in `cobolt-parser/tests/test_nested_programs.rs`.

### Tests

- Added unit/behavioural tests: `fonts::tests` (enumeration, fallback, on-demand
  load, bitmap rejection), `designer::form_resize_tests`,
  `designer::anim_behavior_tests::scale_rect_…`, and `editor::goto_tests`.
  `cargo test -p cobolt-ide` → 35 passing.

## [2.5.0] — 2026-05-30

### Phase 11 — Embed+Bundle Binary Compiler

Cobolt projects can now be compiled into a **single self-contained native
executable** with no source code included.  The output binary embeds the
compressed AST and all form files, then runs them through the existing
interpreter at launch.

#### New crate: `cobolt-compiler`

The core build pipeline lives in `crates/cobolt-compiler/src/lib.rs`:

1. **Load manifest** — reads `cobolt.toml`, resolves main source + additional
   sources + form files.
2. **Lex → parse → semantic** — validates all COBOL sources; aborts on any
   error so only correct programs are compiled.
3. **Serialize + compress** — the `Program` AST is serialised with `bincode`
   and deflate-compressed with `flate2` (best compression).  Typical savings:
   60–75% smaller than raw bincode.
4. **Generate build project** — writes a temporary Cargo project to
   `/tmp/cobolt-build-<name>/` containing:
   - `Cargo.toml` — depends on `cobolt-runtime`, `cobolt-forms`, `eframe`/`egui`
     via path references to the local workspace.
   - `src/main.rs` — embeds assets via `include_bytes!`, contains a lazy form
     dispatch table, and launches either a headless interpreter or an eframe
     form application depending on whether forms are present.
   - `assets/program.bin` — compressed AST.
   - `assets/forms/<ID>.cfrm` — raw form XML for each form.
5. **`cargo build --release`** — compiles the generated project to a native binary.
6. **Copy to `bin/`** — the executable is placed at
   `<project-root>/bin/<project-name>` (`bin/<name>.exe` on Windows) with
   executable permissions set on Unix.

New workspace dependencies: `bincode = "1"`, `flate2 = "1"`.

#### Lazy form loader

The generated binary contains a `static FORMS: &[(&str, &[u8])]` dispatch
table.  A form is only deserialised from its embedded bytes when first
requested by the running COBOL program, keeping startup time constant
regardless of how many forms the project contains.

#### `cobolt build` CLI command

```
cobolt build [cobolt.toml] [--quiet]
```

Calls `cobolt_compiler::build_project()` and prints a summary on success:

```
✅ Build complete!
   Binary : myapp/bin/myapp
   Sources: 3
   Forms  : 2
   AST    : 8 412 bytes (compressed)
```

#### IDE — 🔨 Build Binary menu item

`File → 🔨 Build Binary (bin/)` triggers `do_build_binary()`, which:
- Spawns the compiler on a background thread (IDE stays responsive).
- Shows a ⏳ spinner label while building.
- Prints the binary path and stats in the Output panel when done.
- Shows an error message if the build fails.

---

## [2.4.0] — 2026-05-30

### Phase 10 — REST Client Runtime

COBOL programs can now make real HTTP requests — GET, POST, PUT, DELETE — using
standard `CALL` statements handled entirely inside the interpreter.  No external
tools, FFI, or async runtime are required.

#### New dependency: `ureq` (`cobolt-runtime/Cargo.toml`)

`ureq = { version = "2", features = ["json"] }` — a minimal blocking HTTP
client with built-in TLS support.  No async executor is pulled in.

#### New: `HttpClient` (`cobolt-runtime/src/http_runtime.rs`)

`HttpClient` manages per-session HTTP state for the interpreter:

- `get(url) -> (body, status)` — HTTP GET; returns the response body and
  numeric status code.  On network failure status is `0`.
- `post(url, body) -> (body, status)` — HTTP POST; Content-Type defaults to
  `application/json` unless overridden by `set_header`.
- `put(url, body) -> (body, status)` — HTTP PUT with the same body semantics.
- `delete(url) -> (body, status)` — HTTP DELETE.
- `set_header(name, value)` — adds / overwrites a persistent header sent on
  every subsequent request.
- `clear_headers()` — removes all persistent headers.

All methods strip trailing COBOL spaces from URL and body arguments before
sending.

#### Updated: `Interpreter` — 6 HTTP built-in `CALL` handlers

An `http: HttpClient` field is now part of `Interpreter` (initialised in
`new()`, inherited by `new_with_debug_channels()`).  `exec_call()` handles:

| CALL name                  | Arguments (BY REFERENCE)                          |
|----------------------------|---------------------------------------------------|
| `COBOL-HTTP-GET`           | url-var, response-var, status-var                 |
| `COBOL-HTTP-POST`          | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-PUT`           | url-var, body-var, response-var, status-var        |
| `COBOL-HTTP-DELETE`        | url-var, response-var, status-var                 |
| `COBOL-HTTP-SET-HEADER`    | name-var, value-var                               |
| `COBOL-HTTP-CLEAR-HEADERS` | (no arguments)                                    |

`response-var` receives the full response body (truncated by the `PIC X(32767)`
declaration if needed).  `status-var` (PIC 9(4)) receives the HTTP status code.

#### Updated: Codegen REST stubs (`cobolt-codegen/src/lib.rs`)

The working-storage section for `RestClient` controls no longer uses INVOKE /
OO-style comments.  Generated variables are now:

```cobol
01 WS-REQUEST-URL        PIC X(2048)  VALUE SPACES.
01 WS-REQUEST-BODY       PIC X(32767) VALUE SPACES.
01 WS-HTTP-RESPONSE      PIC X(32767) VALUE SPACES.
01 WS-HTTP-STATUS        PIC 9(4)     VALUE 0.
01 WS-HTTP-HEADER-NAME   PIC X(128)   VALUE SPACES.
01 WS-HTTP-HEADER-VALUE  PIC X(512)   VALUE SPACES.
01 WS-JSON-KEY           PIC X(256)   VALUE SPACES.
01 WS-JSON-VALUE         PIC X(4096)  VALUE SPACES.
```

`write_rest_client_stubs()` now generates three CALL-based paragraphs per
RestClient control (replacing the `INVOKE`-based stubs):

- **`{ID}-GET`** — `CALL "COBOL-HTTP-GET"` with url, response, and status;
  dispatches to the response or error handler paragraph based on the status code.
- **`{ID}-POST`** — `CALL "COBOL-HTTP-POST"` with url, body, response, status.
- **`{ID}-PUT`** — `CALL "COBOL-HTTP-PUT"` with url, body, response, status.
- Response / error handler stub paragraphs are generated for each control.
- An optional `{ID}-SYNC-ITEMS` paragraph copies `WS-HTTP-RESPONSE` and
  `WS-HTTP-STATUS` into user-configured `ResponseDataItem` / `StatusDataItem`
  data fields.

---

## [2.3.0] — 2026-05-30

### Phase 9 — Project Packaging

Cobolt projects can now be bundled into a self-contained, runnable zip archive
both from the IDE and from the command line.

#### New: `cobolt package` CLI command (`cobolt-cli/src/main.rs`)

```
cobolt package [cobolt.toml] [--output path.zip]
```

- Reads a `cobolt.toml` project manifest (defaults to `./cobolt.toml`).
- Packs all tracked source files, forms, and assets with their relative paths
  preserved inside the archive.
- Generates a `run.sh` (Unix, executable) and `run.bat` (Windows) launcher
  so users can run the project without knowing `cobolt` CLI syntax.
- Generates a `README.txt` with installation instructions.
- If a `cobolt` / `cobolt.exe` binary is found next to the currently running
  executable it is automatically bundled, making the archive fully self-contained.
- `--output` / `-o` flag overrides the default output path (`<name>.zip`).
- Prints per-file progress, warnings for missing files, and a final summary.

New dependencies added to `cobolt-cli/Cargo.toml`:
`serde = { workspace = true }`, `toml = { workspace = true }`,
`zip = { version = "2", features = ["deflate"] }`.

#### New: `package_project()` (`cobolt-ide/src/project_model.rs`)

The same packaging logic is available as a library function consumed by the IDE:

- `package_project(project, project_dir, output_zip) -> Result<usize, ProjectError>`
  — packs all tracked files + launchers + README; returns the count of archived items.
- `find_cobolt_binary()` — looks for the runtime binary next to the IDE executable.

#### Updated: IDE — File → Package Project menu item

`CoboltApp::do_package_project()` wires the menu entry to `package_project()`:

- Opens a native Save dialog pre-filled with `<project-name>.zip`.
- Requires a project to be open; otherwise shows a helpful status message.
- Reports the file count and output path in the Output panel on success.

---

## [2.2.0] — 2026-05-30

### Phase 8 — Database Runtime Engine

COBOL programs can now open real SQLite databases, execute SQL, and iterate
over result sets — all from standard `CALL` statements.  No host-language
embedding or FFI required.

#### New dependency: `rusqlite` (`cobolt-runtime/Cargo.toml`)

`rusqlite = { version = "0.31", features = ["bundled"] }` — SQLite is compiled
in from source; no system library or external install is needed.

#### New: `DbConn` and `DbRegistry` (`cobolt-runtime/src/db_runtime.rs`)

`DbConn` wraps a `rusqlite::Connection` and a cached result-set cursor:

- `open(conn_str)` — accepts a bare file path, `sqlite:<path>`, or `:memory:`.
- `exec(sql)` — auto-detects `SELECT`/`WITH`/`PRAGMA` vs. DML.  SELECT results
  are cached as `Vec<Vec<String>>`; DML returns the affected-row count.
- `fetch_col(col)` — returns column `col` (1-based) of the current row.
- `next_row()` — advances the cursor; returns `false` when exhausted.
- `row_count()` / `is_exhausted()` — query result-set metadata.

`DbRegistry` manages all open connections for one interpreter instance as a
`HashMap<u32, DbConn>` keyed by integer *handle*:

- `open(conn_str) -> u32` — opens a connection and returns its handle.
- `exec(handle, sql)`, `fetch_col(handle, col)`, `next_row(handle)`,
  `row_count(handle)`, `is_exhausted(handle)`, `close(handle)`, `close_all()`.

#### Updated: `Interpreter` — 6 SQL built-in `CALL` handlers

A `db: DbRegistry` field is now part of `Interpreter`.  `exec_call()` handles
six new built-in names (matched case-insensitively):

| CALL name            | Arguments (BY REFERENCE)                                  |
|----------------------|-----------------------------------------------------------|
| `COBOL-OPEN-DB`      | conn-string, handle-var (PIC 9(9)), status-var (PIC X)    |
| `COBOL-EXEC-SQL`     | handle, query, row-count-var, status-var                  |
| `COBOL-FETCH-ROW`    | handle, col-index (1-based), dest-var, status-var         |
| `COBOL-NEXT-ROW`     | handle, more-flag-var (`Y`/`N`)                           |
| `COBOL-ROW-COUNT`    | handle, count-var                                         |
| `COBOL-CLOSE-DB`     | handle                                                    |

On interpreter shutdown (`send_debug_finished`) `db.close_all()` is called
to release all connections.

#### Updated: Codegen SQL stubs (`cobolt-codegen/src/lib.rs`)

Working-storage for `SqlDatabase` controls no longer uses `USAGE IS OBJECT`
items.  The generated variables are now:

```cobol
01 WS-{ID}-CONN-STRING   PIC X(512)   VALUE ':memory:'.
01 WS-{ID}-HANDLE        PIC 9(9)     VALUE 0.
01 WS-{ID}-STATUS        PIC X(512)   VALUE SPACES.
01 WS-SQL-QUERY           PIC X(4096)  VALUE SPACES.
01 WS-SQL-ERROR            PIC X(512)   VALUE SPACES.
01 WS-SQL-ROW-COUNT        PIC 9(9)     VALUE 0.
01 WS-SQL-COL-INDEX        PIC 9(4)     VALUE 1.
01 WS-SQL-CURRENT-VALUE    PIC X(512)   VALUE SPACES.
01 WS-SQL-MORE             PIC X(1)     VALUE 'N'.
```

`write_sql_stubs()` generates four CALL-based paragraphs per control:

- **`{ID}-CONNECT`** — `CALL "COBOL-OPEN-DB"` with conn-string, handle, status.
- **`{ID}-EXEC`** — `CALL "COBOL-EXEC-SQL"` with handle, query, row-count,
  status; initialises `WS-SQL-MORE` to `'Y'`.
- **`{ID}-FETCH-ALL`** — loops `PERFORM UNTIL WS-SQL-MORE = 'N'` calling
  `COBOL-FETCH-ROW` for each column index and `COBOL-NEXT-ROW` to advance.
- **`{ID}-CLOSE`** — `CALL "COBOL-CLOSE-DB"` with handle.

---

## [2.1.0] — 2026-05-30

### Phase 7 — Debugger

The IDE now has a full interactive debugger for COBOL programs.

#### New: `DebugCmd` and `DebugEvent` channel types (`cobolt-runtime/src/debugger.rs`)

Two typed enums cross the thread boundary between the IDE and the interpreter:

- **`DebugCmd`** — `Continue`, `StepOver`, `Pause` — sent from the IDE to the
  interpreter to control execution.
- **`DebugEvent`** — `Paused { line, col, paragraph, vars }`, `Resumed`,
  `Finished` — sent from the interpreter back to the IDE.
- **`Breakpoints`** (`Arc<Mutex<HashSet<u32>>>`) — a thread-safe shared set of
  active breakpoint line numbers, written by the IDE and read by the interpreter.

#### Updated: `Interpreter` — per-statement debug hook

`Interpreter::new_with_debug_channels()` is a new constructor that wires the
debug channels into the interpreter.  Before every statement `exec_stmts()` now
calls `debug_check()`, which:

1. Extracts the statement's source line via `Stmt::span()`.
2. Checks whether the line matches a breakpoint **or** `debug_stepping` is true
   (StepOver mode).
3. If a pause condition is met, sends `DebugEvent::Paused` with a complete
   variable snapshot (`CobolEnvironment::iter()` → `VarSnapshot` list) and
   **blocks** on `debug_cmd_rx.recv()` until the IDE sends `Continue` or
   `StepOver`.
4. An async `Pause` command is handled via a non-blocking `try_recv()` poll on
   every statement when not already paused.
5. `DebugEvent::Finished` is sent when `run()` exits normally or via STOP RUN.

`current_paragraph` is updated as each paragraph is entered, so the Paused event
always carries the correct paragraph name.

#### New: `DebugRunner` (`cobolt-ide/src/runner.rs`)

`DebugRunner` is a sister to `Runner` that manages one debug session:

- `start(file_name, source)` — runs the full lex → parse → semantic pipeline,
  then spawns `Interpreter::new_with_debug_channels()` in a background thread.
- `send_cmd(DebugCmd)` — forwards a step/continue/pause command to the thread.
- `drain_events() -> Vec<DebugEvent>` — collects pending debug events each frame.
- `drain_run() -> Vec<RunMsg>` — collects pending run messages (diagnostics,
  output, finished).
- `pub breakpoints: Breakpoints` — the IDE writes breakpoint lines here before
  calling `start()`; the shared pointer is passed directly to the interpreter.
- `stop()` — drops `cmd_tx` (which unblocks any `recv()` in the interpreter,
  causing `Err(_)` → `StopRun`), then joins the thread.

#### New: Debugger side panel (`cobolt-ide/src/panels/debugger.rs`)

`DebuggerPanel` renders in a resizable right-side panel while a debug session
is active:

- **Step toolbar** — ▶ Continue (F5), ⤵ Step Over (F10), ⏸ Pause.  Buttons
  are disabled when the interpreter is running (not paused).
- **Location indicator** — paragraph name and source line, with a colour-coded
  ● Running / ● Paused status indicator.
- **Variable watch table** — displays all `CobolEnvironment` data items as
  a two-column striped grid (name / value), searchable via a filter text box.

#### New: Breakpoint gutter in editor.rs

The code editor's line-number column is now a fully interactive breakpoint
gutter:

- **Click** any line number to toggle a red breakpoint circle (●) on that line.
- When the debugger pauses, a **yellow arrow (→)** and highlighted row mark the
  current execution line.
- `EditorPanel::breakpoints: HashMap<PathBuf, HashSet<u32>>` stores active
  breakpoints per file.
- `breakpoints_for(path)` returns the line set for a given file, used by
  `do_debug()` to initialise the shared `Breakpoints` before starting the session.

#### New: 🐛 Debug toolbar button and keyboard shortcuts

A secondary toolbar strip appears below the main toolbar:

- **🐛 Debug** — starts a debug session for the active file (disabled while a
  normal run is active).  Automatically syncs breakpoints from the editor gutter
  into `DebugRunner::breakpoints` before starting.
- **■ Stop Debug** — drops the command channel (graceful stop), resets the
  debugger panel, and clears the editor debug-line highlight.
- **F5** — Continue (while a session is active).
- **F10** — Step Over (while a session is active).

#### i18n additions (all 5 languages)

New keys: `panel_debugger`, `dbg_continue`, `dbg_step_over`, `dbg_pause`,
`dbg_stop`, `dbg_variables`, `dbg_filter_hint`, `dbg_debug`.

---

## [2.0.0] — 2026-05-29

### Phase 6 — Form Runtime Engine

Forms can now be **executed interactively** from inside the IDE.  Pressing the
new **▶ Run Form** button in the designer toolbar compiles the form's generated
COBOL and runs it in a live, interactive OS window — no external tools required.

#### New: `FormEvent` and `StateUpdate` channel types (`cobolt-runtime`)

`crates/cobolt-runtime/src/channels.rs` introduces two typed messages that cross
the thread boundary between the egui UI and the background interpreter:

- **`FormEvent`** — sent from the UI thread to the interpreter when the user
  interacts with a control (`click()`, `change()`, `got_focus()`, `lost_focus()`).
  A special `quit()` sentinel (`ctrl_id = "__QUIT__"`) is used to unblock and
  terminate the interpreter cleanly.
- **`StateUpdate`** — sent from the interpreter to the UI whenever
  `COBOL-SET-PROPERTY` executes, carrying `ctrl_id`, `prop`, and `value` so the
  UI can update the live control snapshot immediately.

#### Updated: `Interpreter` — GUI channel support

`Interpreter::new_with_channels()` is a new constructor that wires three
`mpsc` channels into the interpreter for GUI-mode execution:

- `event_rx: Receiver<FormEvent>` — **`COBOL-WAIT-EVENT`** now _blocks_ on this
  receiver instead of immediately setting `COBOL-QUIT = 1`, enabling a real COBOL
  event loop.  Receiving the quit sentinel sets `COBOL-QUIT = 1` and exits.
- `state_tx: Sender<StateUpdate>` — **`COBOL-SET-PROPERTY`** sends a
  `StateUpdate` through this channel in addition to writing to the ObjectRegistry,
  so property changes are reflected in the UI on the next frame.
- `display_tx: Sender<String>` — **`DISPLAY`** statements route their output
  through this channel instead of stdout when in GUI mode; the IDE output panel
  receives each line via `OutputPanel::push_line()`.

CLI-mode behaviour (channels `None`) is completely unchanged.

#### New: `FormRuntime` (`cobolt-ide`)

`crates/cobolt-ide/src/form_runtime.rs` manages one live COBOL form execution:

- `FormRuntime::launch(form, form_path)` — generates COBOL from the form model,
  lexes, parses, and runs semantic analysis, then spawns
  `Interpreter::new_with_channels()` in a background thread.  Returns `Err` if
  parse or semantic analysis fails, displaying the errors in the output panel.
- `send_event(FormEvent)` — forwards a UI event to the interpreter thread.
- `drain_state() -> bool` — drains all pending `StateUpdate` messages and applies
  them to the `ctrl_state` snapshot; returns `true` when the UI should repaint.
- `drain_display() -> Vec<String>` — collects all `DISPLAY` lines produced since
  the last frame.
- `is_running() -> bool` — checks whether the interpreter thread is still alive.
- `stop()` — sends the quit sentinel and joins the thread.
- `Drop` impl ensures `stop()` is always called when the runtime is released.

Two supporting types are also defined here:

- **`CtrlMeta`** — immutable snapshot of a control's type, rect, z-order, and
  animations (populated at launch and used only for rendering order).
- **`CtrlState`** — mutable per-control state (`props`, `visible`, `enabled`),
  updated in-place by `drain_state()`.

#### New: **▶ Run Form** / **■ Stop Form** toolbar button

The designer toolbar now shows a **▶ Run Form** button when the form is not
running, and a **■ Stop Form** button while a runtime is active for that form.

- **▶ Run Form** saves the form, calls `FormRuntime::launch()`, and adds the
  runtime to `CoboltApp::form_runtimes`.
- **■ Stop Form** calls `stop()` on the matching runtime and removes it from the
  list.
- Multiple forms can run simultaneously in separate windows.

#### New: live interactive form viewport (`show_running_form_window`)

Each running `FormRuntime` gets its own OS window via `show_viewport_immediate`.
Every frame:

1. `drain_display()` output is forwarded to the IDE output panel.
2. `drain_state()` applies property updates to the live snapshot.
3. Controls are rendered in `z_order` from `ctrl_state` — buttons, labels,
   text boxes, checkboxes, combo boxes, list boxes, sliders, progress bars, and
   image controls are all handled.
4. User interactions fire the corresponding `FormEvent` back to the interpreter
   (`Click`, `Change`, `GotFocus`, `LostFocus`).
5. Non-visual controls (Timer, AgentObject, SqlDatabase, RestClient) are skipped.
6. Closing the window sends `FormEvent::quit()`, which unblocks
   `COBOL-WAIT-EVENT` and terminates the interpreter thread cleanly.

`ctx.request_repaint()` is called every frame while any form runtime is active,
ensuring the UI stays responsive to interpreter-driven state changes.

#### Output panel — `push_line()`

`OutputPanel::push_line(impl Into<String>)` was added to accept plain DISPLAY
output routed from the form runtime engine, displayed in the same monospace
light-grey style as normal program output.

---

## [1.1.0] — 2026-05-29

### New features & fixes

#### Form Designer — Save-on-close guard

Closing a dirty form designer window (one with unsaved changes) now triggers a
**Save / Discard / Cancel** confirmation dialog instead of silently discarding work:

- When the user clicks the OS close button (×) on a designer viewport that has
  unsaved changes, `ViewportCommand::CancelClose` is sent back to the OS to
  prevent the window from disappearing immediately
- A centred modal dialog appears with three choices:
  - **💾 Save & Close** — saves the `.cfrm` file and regenerates the `.cbl` COBOL
    source, then closes the window
  - **🗑 Discard & Close** — closes the window without saving
  - **Cancel** — dismisses the dialog, leaving the designer open and unchanged
- Closing via the dialog's own × button is treated as Cancel
- Clean (non-dirty) windows still close immediately without prompting

#### Form Designer — Save always regenerates COBOL

The **💾 Save** button in the designer toolbar now saves the `.cfrm` form file
**and** regenerates the `.cbl` COBOL source in a single action, keeping both files
in sync at all times.  The hover tooltip reads "Save form and regenerate COBOL".

Previously, Save only wrote the `.cfrm`; the user had to click "⚙ Generate COBOL"
separately to update the COBOL output.

#### Form Designer — Cmd+S in the designer window

**Cmd+S** (or Ctrl+S on Windows/Linux) now works inside designer viewport windows,
triggering the same save + regenerate action as the toolbar button.  Previously
Cmd+S was only handled in the main code-editor window and had no effect when the
designer was focused.

#### Properties panel — SqlDatabase `AutoConnect` type fix

`AutoConnect` was being pushed as `PropValue::String("true"/"false")` instead of
`PropValue::Bool(true/false)`.  The checkbox read the value back via `as_bool()`,
which checks for the `Bool` variant, so toggling `AutoConnect` had no effect.
Fixed: `PropValue::Bool(v)` is now used.

#### Properties panel — SqlDatabase COBOL Data Items grid layout

The "SQL Database — COBOL Data Items" section used an `egui::Grid` with
`num_columns(2)` but each `text_row_hint` call adds only one cell (a horizontal
layout containing both label and field).  The cells were therefore shifted by half
a column, causing labels and text edits to land in the wrong positions.  Fixed by:

- Changing the grid to `num_columns(1)` (each item gets its own full-width row)
- Adding `ui.end_row()` after each of the five `text_row_hint` calls
  (ConnDataItem, ResultSetDataItem, ConnectPara, QueryCompletePara, ErrorPara)

The same missing `ui.end_row()` was also present for the `ConnectionString` row
inside the "SQL Database — Connection" grid; that is fixed too.

#### Format painter — geometry copy

**Copy Style / Paste Style** (🖌 Format Painter) now also copies the source
control's position and size (X, Y, Width, Height) to the target control.

- `FormatPainter::WaitingForTarget` gains a `src_rect: cobolt_forms::model::Rect`
  field that captures the source control's `rect` at copy time
- The paste step writes `tgt.rect = src_rect` alongside the visual style properties
  and animations, so the target control becomes an exact geometric and visual copy
  of the source

#### Dead code removal — `bind_event` / `set_event_code` wiring

Removed all remnants of the old inline-editor event wiring that was superseded by
the modal `EventEditorModal` in v1.0.0:

- `pub bind_event: Option<(String, String, String)>` field removed from
  `InspectorAction` (was always `None` after the modal refactor)
- `bind_event()` and `set_event_code()` methods removed from `DesignerPanel`
- The three-line `bind_event` dispatch block removed from `DesignerPanel::handle_drag`

#### Label word wrap

Labels whose `Caption` text exceeded the control width were bleeding outside the
control border.  Two bugs were fixed:

1. **Wrong `max_width`** — `LayoutJob::wrap.max_width` was not set, so egui laid
   out the text as a single infinite line
2. **Wrong anchor for centred text** — with `halign = Align::Center`,
   `painter.galley(pos, ...)` treats `pos` as the **top-centre** anchor, not
   top-left.  `text_pos.x` was being set to `rect.min.x` (left edge), shifting
   the entire text block half a control-width to the left.  Fixed to
   `rect.center().x`.

#### IntelliSense — selection on click and Tab

Three bugs prevented selecting an autocomplete suggestion:

1. **Popup dismissal race** — `else { self.ac.visible = false; }` ran on the same
   frame the user clicked a row (the click briefly steals `TextEdit` focus, making
   `cursor_range` return `None`); the popup vanished before the click was processed.
   Fixed by removing the `else` branch entirely — the popup is now only dismissed
   by an explicit selection or Escape.

2. **Click detection on `Frame` rows** — `row_resp.response.interact(Sense::click())`
   does not detect clicks on `egui::Frame` responses because frames only sense hover.
   Fixed by replacing with `ui.interact(rect, id, Sense::click())`.

3. **Char vs byte index mismatch** — `trigger_pos` is a char index returned by
   `word_before_cursor`, but it was used directly as a byte offset in
   `String::replace_range`, causing a panic or wrong replacement on non-ASCII input.
   Fixed by converting via `tab.content.char_indices().nth(self.ac.trigger_pos)`.

#### Pointing-hand cursor on clickable elements

All interactive elements that use custom interaction (not standard egui buttons or
selectable labels) now show the `PointingHand` cursor on hover:

- **Toolbox cells** — `ui.ctx().set_cursor_icon(CursorIcon::PointingHand)` on hover
- **Canvas controls** — pointer becomes a hand when hovering any placed control
- **Properties panel event rows** — `.on_hover_cursor(CursorIcon::PointingHand)`
  on both control-event and form-event rows
- **Autocomplete popup rows** — `.on_hover_cursor(CursorIcon::PointingHand)` via
  the `click_resp` interact result

---

## [1.0.0] — 2026-05-29

### Major — Nested-program architecture

This is the first major version bump.  The entire code generation and form storage
model has been redesigned: each event handler becomes a COBOL-85 nested
program; the `.cfrm` file is the single source of
truth; the generated `.cbl` is a build artifact the user never edits.

#### `.cfrm` file format (v1.0 — backward-compatible load)

Three new XML sections added to `.cfrm`:

- `<working-storage><![CDATA[...]]></working-storage>` — raw COBOL data declarations
  emitted verbatim into the outer program's WS; supports `GLOBAL` and `EXTERNAL`
  clauses for form-wide and cross-form data sharing
- `<form-events>` — `OnLoad` and `OnClose` lifecycle handlers stored as `<Event>`
  children with CDATA bodies
- `<deleted-controls>` — recycle bin: event code from deleted controls preserved
  here (never emitted into `.cbl`) so it can be restored later

`<Event>` elements now use start/end form with CDATA body for the user's COBOL
statements.  Old-format self-closing `<Event .../> ` tags still load correctly
(`code` will be empty).

#### Model changes (`cobolt-forms`)

- `EventBinding` gains `code: String` — raw COBOL statements for this handler
- `EventBinding::for_control(ctrl_id, event)` — auto-derives paragraph name as
  `"CTRL-ID--EVENT-NAME"` (double-hyphen separator)
- `EventBinding::has_code()`, `code_line_count()` — UI helpers
- `derive_paragraph_name(ctrl_id, event) -> String` — public utility function
- `Form` gains `user_ws_source: String`, `form_events: Vec<EventBinding>`,
  `deleted_code: Vec<DeletedControlCode>`
- `Form::new()` pre-populates `form_events` with empty `OnLoad` / `OnClose` stubs
- `Form::recycle_control(id, timestamp)` — moves event code to recycle bin before
  deleting; `restore_from_recycle(timestamp, target_id)` recovers it
- `Form::control_has_code(id)` — returns `[(event, line_count)]` for UI dialog
- `Control::ensure_event(event)` — idempotent event binding with auto-derived name
- `DeletedControlCode` struct — `control_id`, `deleted_at` (ISO timestamp), `events`

#### Properties panel (`cobolt-ide`)

- "Event Bindings" section replaced by read-only "Events" section showing `●`/`○`
  status dots and line counts per supported event; user directed to Code View to edit
- "COBOL Paragraphs" section removed from chart controls (superseded by Code View)
- `new_ev_name` / `new_ev_para` staging fields removed from `PropertiesPanel`

#### Code generation (`cobolt-codegen`) — Phase 2 complete

- `write_procedure_division()` fully rewritten to emit COBOL-85 nested-program structure
- Outer program (`COBOL-MAIN`) calls `CALL "MAIN-FORM--ON-LOAD"` / `CALL "MAIN-FORM--ON-CLOSE"` for lifecycle events; event loop dispatches to handlers via `CALL "BTN-OK--CLICK"` (not `PERFORM`)
- New `write_nested_programs()` iterates form-level events then per-control events and emits a nested program for each
- New `write_nested_program(prog_id, code, comment)` emits a self-contained `IDENTIFICATION … PROCEDURE … GOBACK. END PROGRAM name.` block; empty handlers get `CONTINUE.` with a TODO comment
- Outer program closes with `END PROGRAM <form-name>.`
- Tests updated: `generate_contains_nested_program`, `generate_contains_form_events_nested`, `generate_calls_on_load_nested`

#### Backward-compatibility removal (`cobolt-forms`)

- `Form::load_paragraph` and `Form::close_paragraph` fields removed
- `OwnedEvent::EventEmpty(String, String)` variant removed
- `load-paragraph` / `close-paragraph` attributes removed from XML save/load
- `backward_compat_empty_event_tag` test removed
- `PropertiesPanel` "On Load" / "On Close" paragraph text-edit rows removed
- `set_form_prop("LoadPara")` / `set_form_prop("ClosePara")` arms removed from designer
- Raw string delimiter in XML test changed from `r#"..."#` to `r##"..."##` (fix: `"#FFFFFF"` terminated the former prematurely)

#### IDE — Interactive event code editor (interim, Phase 5 preview)

- Events section in Properties panel replaced by a collapsible inline COBOL editor per event
- Each event row shows a `▸`/`▾` arrow, `●`/`○` code-presence dot, and line count
- Expanding a row shows the derived `PROGRAM-ID` hint and a 6-row monospace `TextEdit`
- Edits are propagated back to `EventBinding.code` via `InspectorAction::set_event_code`
- `#[derive(Default)]` added to `InspectorAction`; `set_event_code: Option<(String,String,String)>` field added

#### Toolbox icon size

- Icon buttons enlarged from 39 × 39 px to 49 × 49 px (+25 %)
- Top and left padding increased from 5 px to 10 px (+5 px each)

#### Parser — Phase 3: COBOL-85 nested program support

- `cobolt-lexer`: added `Token::End` for the bare word `"END"` (distinct from `END-IF`, `END-PERFORM`, etc.)
- `cobolt-ast/DataDecl`: added `is_global: bool` and `is_external: bool` fields
- `cobolt-ast/Program`: added `nested_programs: Vec<Program>` and `end_program_name: Option<String>` fields
- `cobolt-parser/data.rs`: `GLOBAL` and `EXTERNAL` clauses now set flags on `DataDecl` instead of being silently skipped; `Token::End` added to all stop-condition lists so data parsing halts before `END PROGRAM`
- `cobolt-parser/procedure.rs`: `Token::End` added to every stop condition in `parse_sections`, `parse_paragraphs_until_section`, `parse_paragraphs`, and the `parse_stmts` stop closures so paragraph/section collection halts before `END PROGRAM`
- `cobolt-parser/parser.rs`: `parse_program` delegates to new free function `parse_single_program`; after the `PROCEDURE DIVISION` the function loops collecting nested programs (each starting at `IDENTIFICATION`) and terminates on `END PROGRAM name.` or EOF; nested programs are stored in `Program::nested_programs`
- `cobolt-ast` tests updated with `is_global`, `is_external`, `nested_programs`, `end_program_name` fields

#### Runtime (`cobolt-runtime`) — Phase 4 complete

**`CobolEnvironment` scope management**

- `push_local_scope(items)` — inserts a nested program's own WORKING-STORAGE
  items into the shared env store and returns the list of keys that were newly
  added (items that already exist, e.g. GLOBAL names, are not overwritten)
- `pop_local_scope(keys)` — removes those keys on GOBACK, restoring the env
  to its pre-call state
- `global_items_from_data_division(data)` — collects all `is_global`-flagged
  data items from a DATA DIVISION; utility used internally by the registry builder

**`Interpreter` nested-program registry**

- New `NestedProgram` struct — holds `para_map`, `para_order`, and
  `local_items: Vec<(String, CobolValue)>` for one nested program
- New `nested_registry: HashMap<String, NestedProgram>` field on `Interpreter`
- `register_nested(prog, registry)` — free function that recursively registers a
  `Program` and all of its `nested_programs` into the registry (keyed by
  PROGRAM-ID, uppercase); called from `Interpreter::new()` at startup
- New `run_para_sequence(para_map, para_order)` method — executes a paragraph
  sequence from an explicit map (not `self.para_map`); handles GO TO within
  the nested program's own paragraph space; GOBACK propagated to caller

**`exec_call` dispatch**

- Added `_ if self.nested_registry.contains_key(&prog_name)` arm before the
  legacy flat-paragraph fallback
- On match: clones para_map + para_order + local_items out of registry (to
  avoid simultaneous mutable borrow), calls `push_local_scope`, runs
  `run_para_sequence`, calls `pop_local_scope` even on error
- GOBACK from a nested program is treated as a normal return (not an error)
- GLOBAL items from the outer program are naturally visible to nested programs
  because they live in the same `CobolEnvironment` store — no copying needed

**Tests** — `tests/test_nested_programs.rs`

- `call_nested_program_runs_and_returns` — CALL dispatches, nested program sets outer WS, returns
- `nested_local_ws_is_removed_after_goback` — local items do not persist after GOBACK
- `global_items_shared_with_nested_program` — GLOBAL WS mutations are visible in outer env
- `nested_program_internal_goto` — GO TO works within nested para_map; does not escape
- `multiple_nested_programs_dispatch_independently` — each CALL routes to the right program
- `nested_program_without_end_program_terminator` — unterminated last nested program still callable

#### IDE — modal event code editor — Phase 5 complete

The inline 6-row TextEdit in the Properties panel is replaced by a full-screen modal
editor.

- Clicking any event row (in either the control Properties or the Form Properties
  Events section) opens a centred `egui::Window` overlay
- The modal renders a read-only COBOL scaffold around two editable areas:
  - **WORKING-STORAGE SECTION** — local data items specific to this handler
    (e.g. `01 WS-MY-VAR PIC X(64) VALUE SPACES.`)
  - **PROCEDURE DIVISION body** — the user's COBOL statements
- Read-only scaffold lines are colour-coded (green for structural keywords, gray
  for division headers); editable areas use monospace 12pt with syntax hint text
- **Save** commits both `local_ws` and `code` to the model (dirty-flagged);
  **Cancel** discards changes and closes without writing
- A semi-transparent black overlay dims the canvas behind the modal
- `EventEditorModal` struct added to `designer.rs` with `ctrl_id`, `ctrl_display`,
  `event_name`, `program_id`, `ws_buf`, `proc_buf`, `orig_ws`, `orig_proc`, `saved`
- `DesignerPanel::open_event_modal(ctrl_id, event_name)` — opens the modal,
  pre-populating buffers from the model (or blank if the event has no binding yet)
- `DesignerPanel::save_event_handler(ctrl_id, event_name, ws, code)` — writes
  both buffers back into the form, for either control or form-level events
- `DesignerPanel::show_event_modal(ui)` — renders the modal; called at the end
  of `show()` so it floats above all other content

**Model** — `EventBinding` gains `local_ws: String` for per-handler WS declarations;
XML layer extended with `<LocalWS><![CDATA[...]]></LocalWS>` child element inside
`<Event>` (backward compatible: old files without `<LocalWS>` still load correctly);
codegen updated to emit `local_ws` content in the handler's WS section instead of a
placeholder comment.

**Properties panel**
- `selected_event` and `event_code_bufs` fields removed
- `InspectorAction::set_event_code` replaced by `open_event_editor: Option<(String, String)>`
  containing `(ctrl_id, event_name)`; empty `ctrl_id` = form-level event
- Form Properties section gains "⚡ Form Events" subsection with clickable `OnLoad` /
  `OnClose` rows that open the same modal

---

## [0.2.2] — 2026-05-29

### Fix — Chart SET-TABLE generates invalid COBOL when DataSource/DataCount not set

`write_chart_stubs()` used `.map().unwrap_or_else(fallback)` to default empty
DataSource / DataCount properties, but if the property exists as an empty string
`Some("")`, `unwrap_or_else` never fires.  The result was invalid generated COBOL:

```cobol
           MOVE         TO WS-LIN-13-SELECTED-IDX        *> missing source
           CALL "COBOL-CHART-SET-TABLE" USING "LIN-13"   *> missing args
```

Fix: added `.filter(|s| !s.is_empty())` before `unwrap_or_else` so empty strings
fall through to the placeholder-name fallback (`WS-<ID>-TABLE` / `WS-<ID>-COUNT`).
Generated code now compiles cleanly even when the chart has no data binding configured.

---

## [0.2.1] — 2026-05-29

### Fix — Runtime COBOL-* built-in calls not recognised (warn + infinite loop)

After task 64 renamed all generated identifiers from `COBOLT-*` to `COBOL-*`, the
cobolt interpreter's `match` still only recognised the old `COBOLT-WAIT-EVENT` /
`COBOLT-SET-PROPERTY` / `COBOLT-GET-PROPERTY` spellings.  Every generated form
program therefore hit `CALL to unknown program 'COBOL-WAIT-EVENT' — ignored` on
startup, and the event loop would spin forever in CLI mode.

Changes to `cobolt-runtime/src/interpreter.rs`:

- Added `"COBOL-INIT-FORM"` arm — no-op in CLI/non-GUI mode (suppress spurious warn)
- Renamed `"COBOLT-WAIT-EVENT"` → `"COBOL-WAIT-EVENT"` (old spelling kept as alias)
- **`COBOL-WAIT-EVENT` now sets `COBOL-QUIT = 1`** so the event loop exits cleanly
  in CLI mode instead of spinning until the process is killed
- Added `"COBOL-SET-PROPERTY"` / `"COBOL-GET-PROPERTY"` as primary spellings (old
  `COBOLT-*` aliases retained for backward compatibility)
- Added `"COBOL-CHART-SET-TABLE"`, `"COBOL-CHART-ADD-POINT"`, `"COBOL-CHART-CLEAR"`,
  `"COBOL-CHART-REFRESH"` stubs — log at DEBUG level in CLI mode, no warning

---

## [0.2.0] — 2026-05-29

### New feature — Rich chart controls

Six chart control types added to the Form Designer toolbox under a new **Charts**
category.  Charts are first-class form controls that participate in the full designer
workflow: placement on the canvas, property inspection, COBOL code generation, and
XML persistence.

**Control types added**

- `BarChart` — vertical bar chart; default size 320 × 220
- `LineChart` — line/trend chart; default size 320 × 220
- `PieChart` — pie chart; default size 240 × 240
- `AreaChart` — filled area chart; default size 320 × 220
- `ScatterChart` — scatter-plot chart; default size 320 × 220
- `DonutChart` — donut / ring chart; default size 240 × 240

**Data binding**

Charts accept data via two complementary mechanisms:

1. **COBOL table binding** — pass an existing WORKING-STORAGE table and its element
   count directly:
   ```cobol
   INVOKE CHART1 SET-TABLE USING WS-SALES-TABLE WS-SALES-COUNT
   ```
2. **Point-by-point accumulation**:
   ```cobol
   INVOKE CHART1 ADD-POINT USING 'January' WS-MONTHLY-TOTAL
   INVOKE CHART1 CLEAR
   INVOKE CHART1 REFRESH
   ```

**Properties inspector** — dedicated chart section covering:

- *Visual*: Title, ShowLegend, ShowGridLines, ShowTooltips, AnimateOnLoad,
  X-axis / Y-axis labels
- *Data Binding*: DataSource, DataCount, LabelField, ValueFields, SeriesLabels
- *Type-specific*: grouped/stacked bars, smooth/stepped lines, inner-radius for
  donut, log-scale Y axis, bubble size for scatter, fill-opacity for area
- *COBOL Paragraphs*: DataChanged event paragraph stub
- *INVOKE usage hint* displayed inline

**Designer canvas** — glass-styled chart previews rendered with sample data at
design time (bars, polylines, filled polygons, scatter dots, pie/donut fan slices).

**Code generation**

- `WORKING-STORAGE SECTION` — three items per chart:
  `WS-<ID>-SELECTED-IDX` (PIC 9(4)), `-SELECTED-LBL` (PIC X(64)),
  `-SELECTED-VAL` (PIC 9(12)V99)
- `PROCEDURE DIVISION` — four stub paragraphs per chart:
  `<ID>-SET-TABLE`, `<ID>-ADD-POINT`, `<ID>-CLEAR`, `<ID>-REFRESH`

**Toolbox** — hand-drawn vector icons for all six chart types; unique ID prefixes
(`BAR`, `LIN`, `PIE`, `ARE`, `SCT`, `DNT`).

---

## [0.1.0] — 2026-05-29

### New feature — Snap-to-grid toggle

- Added `snap_to_grid: bool` field to the `Form` model (default `true`); persisted
  as a `snap-to-grid` XML attribute in `.cfrm` files (backward-compatible: missing
  attribute defaults to `true`)
- `snap()` in the designer canvas is now dynamic — it takes `grid_px` and `enabled`
  parameters instead of using a hardcoded 4 px constant; all move/resize/place
  operations respect the per-form setting
- Added **"Snap to grid"** checkbox to the Grid section of Form Properties (sits
  directly below "Grid size"); checking/unchecking takes effect immediately for
  move, resize, and new-control placement
- Updated all `Form` struct literals in test/codegen code to include
  `snap_to_grid: true`

Versioning rules
- **PATCH** (`0.0.x`): bug fixes, polish, build corrections
- **MINOR** (`0.x.0`): new features — resets PATCH to 0
- **MAJOR** (`x.0.0`): any change to the interpreter — resets MINOR and PATCH to 0

---

## [0.0.1] — 2026-05-29  *(initial tagged release)*

### Foundation (pre-tag, post-parser)

All work below was completed before the 0.0.1 tag was applied.
It is catalogued here as the baseline feature set.

---

#### Runtime & Toolchain

- **cobolt-semantic** — semantic analysis crate scaffolded; identifier resolution and
  basic type checking
- **cobolt-runtime / interpreter** — tree-walking interpreter for all AST statement
  types including `Stmt::TryCatch` and `Stmt::Throw` (try/catch/finally semantics,
  `UserException` error variant, exception variable binding)
- **cobolt-stdlib** — standard-library crate with built-in COBOL helper functions
- **cobolt-cli** — command-line binary (`cobolt run <file>`) wrapping the interpreter
- **INVOKE keyword** — added `Token::Invoke` to the lexer and a pass-through
  `Stmt::Invoke` to the parser; codegen emits `INVOKE` correctly
- **PLAY / STOP animation verbs** — `PLAY ANIMATION` / `STOP ANIMATION` statements
  added to lexer and parser
- **TRY / CATCH EXCEPTION / FINALLY** — full exception-handling block added to
  lexer and parser; interpreter executes all three clauses with correct fall-through

---

#### IDE Shell (`cobolt-ide`)

- **eframe/egui shell** — main application window with liquid-glass translucent
  visuals, dark-navy palette, rounded controls, and frosted-glass panel fills
- **macOS dock icon** — programmatically generated 256×256 navy rounded-square
  with a blue "C" arc and terminal serifs
- **Code editor panel** — scrolling source editor, syntax-aware font (12 pt
  monospace), auto-completion stubs, search/replace with focus-restore fix
- **Output / console panel** — scrolling log for run output and diagnostics
- **Project system** — `cobolt.toml` project file, project explorer panel with
  grouped tree view (forms, sources, assets), new-project dialog
- **Run / stop** — background thread runner, real-time output streaming,
  diagnostic markers fed back into the editor
- **Keyboard shortcut handling** — Cmd/Ctrl+S save, Cmd/Ctrl+Z undo,
  Cmd/Ctrl+Shift+Z redo wired globally

---

#### Form Designer

- **cobolt-forms model** — `Form`, `Control`, `ControlRect`, `PropValue`,
  `Animation`, `AnimTrigger`, `AnimEasing`, `BgImageMode` data types;
  XML serialisation/deserialisation (`cobolt-forms/src/xml.rs`)
- **cobolt-codegen** — form-to-COBOL source generator; REST-API stub codegen;
  DataGrid CSV-export stubs; full PROCEDURE DIVISION with all control paragraphs
- **Multi-viewport designer windows** — each open `.cfrm` file gets its own OS
  window via `ctx.show_viewport_immediate`
- **Canvas** — pixel-accurate form canvas with dot grid (configurable density),
  drag-to-place, drag-to-move, rubber-band multi-select, snap-to-grid
- **Control types (29 total)**:
  Button, Label, TextBox, CheckBox, RadioButton, ComboBox, ListBox,
  NumericUpDown, DateTimePicker, GroupBox, Panel, TabControl, Splitter,
  DataGrid, TreeView, PictureBox, ProgressBar, Slider, Line, Shape,
  MenuBar, ToolBar, StatusBar, Timer, AgentObject, RestClient,
  SqlDatabase (non-visual), ModalWindow
- **Vector icon toolbox** — two-column icon grid with hand-drawn vector icons for
  every control type, collapsible categories, live search filter;
  buttons enlarged to 39 × 39 px with 5 px top/right padding
- **Properties inspector** — two-column table layout; universal properties
  (Name, Caption, Position, Size, Font, Colors, Opacity, Transparency, Enabled,
  Visible, Z-Order); per-type sections for every control type;
  `SqlDatabase` connection properties (driver, host, port, database, user,
  password, auto-connect, max connections); panel width capped at 320 px to
  prevent overflow
- **Forms list panel** — sidebar list of all `.cfrm` files in the project root,
  open-on-click
- **Undo / redo stack** — full snapshot-based undo/redo for all designer mutations
- **Alignment toolbar** — align left/right/top/bottom/center-H/center-V,
  bring-to-front/send-to-back, delete selected; double-height toolbar
- **Z-order** — per-control z_order field; `Bring to Front` / `Send to Back`
  commands; canvas renders controls in z-order
- **Multi-select** — rubber-band selection, Shift+click toggle, group move
- **Form background** — solid fill colour (hex picker), transparency slider (0–100 %),
  background image path + stretch/tile/center/fit display modes
- **Grid density** — grid size property (8/16/32 px) on the Form, adjustable in
  Form Properties
- **Animation system** — per-control animation list; properties: name, trigger
  (`OnFormLoad`, `OnClick`, `OnHover`), easing, direction, duration, delay,
  loop count; designer-time live preview with play/stop controls;
  `AnimState` struct tracks t, playing, forward, delay_remaining
- **Preview window** — live OS window (`with_transparent(true)`) showing the form
  with liquid-glass control rendering, per-control opacity/transparency, and
  `OnFormLoad` animations auto-started on open; glass visuals applied to preview
  viewport; main designer visuals restored every frame to prevent bleed-through
- **Delete key guard** — Delete/Backspace only removes selected controls when no
  text-input control has keyboard focus (`ctx.memory focused().is_none()`)
- **Target device presets** — "Target" dropdown in Form Properties with 24 device
  presets (iPhone, iPad, Apple Watch, Android phone/tablet/watch, custom);
  selecting a preset auto-sets form width × height
- **COBOL identifier rename** — `COBOLT-*` data-division identifiers renamed to
  `COBOL-*` throughout codegen and semantic crates

---

*Next version: increment PATCH for fixes, MINOR for new features,
MAJOR for interpreter changes.*
