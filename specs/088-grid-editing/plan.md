# Plan — Grid editing in the Form Designer

- **Status:** approved for `/tasks` (operator, 2026-10-15: D1 — a separate button, and D10 — the form's own grid is a follow-up, both as recommended; the hint line and D11 not objected to)
- **Spec:** ./spec.md (approved by the operator, 2026-10-15; open questions Q1–Q4 taken as proposed)
- **Date:** 2026-10-15

## 0. What reading the code changed

Four facts about the code as it is, which the spec did not know and which shape
the design. Each is a decision below.

1. **There is already a "Toggle Grid" button.** `DesignerPanel.show_grid`
   (default `true`) is the *alignment dots* grid, with its own toolbar button
   and a dotted-grid icon (`icon_grid`, `draw_grid`). The spec's **Show Grid**
   is a different thing — the layout grids of Grid containers — and shares its
   name, its default and its highlighted state. → **D1.**
2. **The layout engine does not say which cell a child is in.** `ContainerGeom`
   carries the laid-out track *sizes* (that is what R45's dashed lines are
   drawn from), but `grid::place` — the function that decides each child's
   cell, auto-placed children included — is private and its answer is thrown
   away. Merge (R16), the "controls in the range" test and R12's notice all
   need it. → **D6.**
3. **Track lists are text, and the parser forgets what it cannot read.**
   `tracks::parse` skips a token it does not understand "so a half-typed value
   never blanks the form". Re-serialising a parsed list would therefore
   silently delete a developer's half-typed token. → **D2.**
4. **A grid can have more tracks than its text lists.** A row that exists only
   because a child was placed in it (`GridRow = 3` on a grid whose `GridRows`
   names two) is an *implicit* `Auto` track. The canvas draws it; the text does
   not hold it. → **D3.**

## 1. Approach

Three layers, bottom up, each testable without the one above it. This is the
shape spec 094 (DataGrid edit mode) already established — pure edit functions
that return property edits, a thin canvas host, one floating toolbar — and the
plan reuses it rather than inventing a second one.

### Layer 1 — `cobolt-forms`: the grid edit model (pure, no egui) *(R10–R17, R19–R21, R23)*

A new module `layout/grid_edit.rs` holds every edit as a **pure function** from
the container's properties and its children's cells to a list of property
writes `Vec<(control id, key, PropValue)>`. Nothing here reads the canvas.

- **Token level, not parse/serialise (D2).** `tracks::split(text)` splits the
  list at top-level spaces (a `MinMax(…)` or `Repeat(…)` stays one token) and
  `tracks::join` puts it back. An edit replaces, inserts or removes whole
  tokens and leaves every other token — including one the parser cannot read —
  byte-for-byte as it was.
- **Operations:** `insert_track` (R10), `remove_track` (R11, R12),
  `restore_track` (R13), `resize_pair` (R19–R21), `merge` (R15, R16),
  `split_cell` (R17). Each returns the writes *and*, for removal, the record
  Restore needs (`RemovedTrack`: axis, index, token, and the placement of each
  child it held).
- **Cells from the solver (D6).** `grid::solve` already runs `place`; it now
  also returns the cell of each item (`Solved.cells`: first row, first column,
  row span, column span, 0-based), and `ContainerGeom` carries them keyed by
  control id when the designer asks for tracks. So the designer reads the same
  placement the runtime uses, auto-placement and implicit tracks included.
- **Implicit tracks (D3).** Before an add, a remove or a resize touches an axis
  whose laid-out track count exceeds the tokens in its text, the missing
  trailing tracks are written out as explicit `Auto` tokens in the same edit.
  Nothing moves — an implicit track *is* `Auto` — and every edit then works on
  an explicit list.
- **Resizing is done in pixels, written in each track's own unit (D8).** The
  drag moves the line by `d` pixels; the two tracks become `a + d` and `b − d`
  (each at least 8 px, R21); every other track keeps its pixel size. Each of the
  two is then *re-expressed* in its unit (R20): `px` → the rounded pixel value;
  `%` → its share of the container's client size; `Auto` → a `px` track; `fr` →
  an `fr` value chosen so the solver gives that pixel size while the other `fr`
  tracks keep theirs (for two `fr` tracks with nothing else flexible this is
  "re-share the combined total", and gives AC10's `0.5fr 1.5fr`). `MinMax(…)`
  and `Repeat(…)` refuse (R20, R14).

### Layer 2 — `cobolt-ide`: the canvas host *(R3, R4, R6–R9, R18, R22–R24, R27–R31)*

A new `panels/layout_grid_edit.rs` — named apart from spec 094's `grid_edit.rs`,
which is about DataGrid controls — holds the view state and the geometry the
gestures are hit-tested against, and `designer.rs` hosts it, in the same places
spec 094's mode is hosted.

- **View state, never saved, never an undo step by itself (R5):** `show` (the
  toggle, default on, **not** persisted — Q1), the cell selection
  (`container`, anchor cell, head cell — R7), the Restore history (D5), and the
  notice shown after a removal (R12).
- **Which grid a press belongs to.** One function answers "the innermost Grid
  container whose client rectangle holds this point" from `canvas_layout`
  (R31's innermost rule falls out of it). The mode is *active* only when the
  form lays out (`Form::lays_out`, spec 056), no other designer mode owns the
  pointer (Preview, Visual Tab Order, the Non-Visuals tab, DataGrid edit mode,
  Format Painter, placing a new control), and the canvas has a Grid container.
- **Show Grid on (R3, R6–R9).** Every Grid container's track lines are painted
  (the R45 painter, generalised from "the selected one"). A press inside a Grid
  container selects the **container** (so the Properties pane shows its
  `GridColumns`, and the toolbar has something to be beside) and a **cell**;
  Shift-click or a drag extends it to a range. The controls inside are not hit.
- **Show Grid off (R22–R24, R27–R31).** Controls select as today. Selecting a
  control that sits in a Grid container additionally paints *that* container's
  lines with the control's cell highlighted, for as long as it stays selected
  (innermost grid, R31); no other grid's lines are drawn.
- **Dragging a line (R18–R21, R28, R29).** A press within a 4 px grab band of an
  inner line takes priority over what is beneath it — the splitter-division and
  sidebar-seam rule (`splitter_division_at`, `sidebar_seam_at`) — but *only*
  within the band (R29). A new `DragState::DraggingGridLine` writes the two
  tracks live, so the canvas re-lays out under the pointer, and on release
  commits **one** undo step (D5), exactly as `MovingSplitterDivision` does.
  The cursor is `ResizeColumn` / `ResizeRow`; a `MinMax`/`Repeat`/zoomed line
  shows none.
- **Resizing a control that sits in a grid (R23, R24).** With Show Grid off, a
  resize handle on such a control no longer writes the control's rectangle: a new
  `DragState::ResizingGridCell` maps the dragged edge to the line before or
  after the control's span and runs the same pair resize (corner handles move
  one line on each axis). The control's own `x/y/w/h` is never written.
- **The floating toolbar (R8, R9).** A foreground `egui::Area` at a fixed
  position and fixed size computed from its button count — the shape of
  `grid_edit::show_toolbar` — beside the container, outside it, clamped to the
  canvas. Icon-only; each button disabled (never hidden) when it does not apply,
  with a tooltip that says why when it is the 088-specific reason (Merge with
  zero or two controls in the range; a `Repeat(…)` axis; the last track).
- **The Show Grid button (R1, R2).** One more `icon_btn` in
  `draw_icon_toolbar`, highlighted while on, with a new painted icon (a framed
  table, two inner lines) so it cannot be mistaken for the dotted "Toggle Grid".

### Layer 3 — commands, undo, Restore *(R13, R25, R26)*

Every edit becomes `Cmd::SetProperty` writes inside one `Cmd::Batch` — which
already exists for exactly this ("a splitter division drag … one gesture, so one
undo"). A removal additionally records its `RemovedTrack` in the designer's
history; keeping that history in step with Undo and Redo is **D5**.

## 2. Affected crates / files

- `crates/cobolt-forms/src/layout/tracks.rs` — `split`, `join`, token readers
  (`kind_of`, `has_repeat`), unit formatting (`fmt_px`, `fmt_fr`, `fmt_percent`).
- `crates/cobolt-forms/src/layout/grid.rs` — `Solved.cells`; `place` result
  returned; no change to how anything is placed.
- `crates/cobolt-forms/src/layout/mod.rs` — `ContainerGeom.cells`, filled where
  `geom.columns`/`rows` already are (line ~812).
- `crates/cobolt-forms/src/layout/grid_edit.rs` — **new**: the pure operations
  and `RemovedTrack`. Re-exported from `layout`.
- `crates/cobolt-ide/src/panels/layout_grid_edit.rs` — **new**: view state, hit
  testing, track-line and cell painting, toolbar, icons.
- `crates/cobolt-ide/src/panels/designer.rs` — host: the toggle field, press
  priority, cursor, the two new `DragState` variants, the paint call beside
  `paint_container_overlay` (which it generalises), `Cmd::GridEdit` handling in
  `execute`/`revert`, the toolbar button and its `DesignerToolbarAction`.
- `crates/cobolt-ide/src/ide_text_data.rs` — the new texts, six languages
  (see D11).
- `crates/cobolt-ide/src/panels/designer_grid_layout_edit_tests.rs` — **new**:
  frame-driven acceptance tests, modelled on `designer_grid_edit_094_tests.rs`.
- `docs/developers-guide-en.md` — "Editing a grid on the canvas", after
  *Responsive examples in PowerDemo3*. Its five translations are deleted per
  GOLDEN RULE #8 (the Guide has only `-en.md` today, so nothing to delete).
- `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` — `z` bump and entry.
- **Not touched:** `cobolt-runtime`, `cobolt-form-host`, `cobolt-cli`,
  `cobolt-compiler`, `cobolt-codegen`, and the System KB (no control, property,
  method or event changes; `ColumnSpan`/`RowSpan` docs are re-read, and the
  store is rebuilt only if they turn out to be wrong).

## 3. Data / model changes

- **`.cfrm`: none.** An edit writes `GridColumns`, `GridRows`, `GridColumn`,
  `GridRow`, `ColumnSpan` and `RowSpan` — properties that already exist, in the
  forms they already have. No schema, no migration, nothing new to load.
- **In memory:** `Solved.cells` / `ContainerGeom.cells` (derived each layout,
  never stored); `Cmd::GridEdit`; two `DragState` variants; the designer's
  `LayoutGridView` (toggle, cell selection, Restore history, notice).
- **A tracks write is minimal:** a drag rewrites the two tokens it moved; an add
  or remove rewrites the one list and the children whose cell numbers change.
  Nothing else on the form is touched, so a saved diff shows only the grid.

## 4. Key decisions & alternatives

- **D1 — The new toggle is a separate button, not the existing "Toggle Grid".**
  *Why:* the alignment dots and the layout grids are independent — dots matter on
  an `Absolute` container, layout grids on a `Grid` one — and the dots are on by
  default *because* a form is usually designed against them. Folding the two
  together would make "hide the dots for a clean screenshot" silently switch the
  canvas into a mode where controls in grids cannot be selected. *Cost:* two
  grid-looking buttons. *Mitigation:* distinct icons (dots vs. a framed table)
  and distinct tips ("Toggle Grid" stays; the new one reads "Show layout grids
  and edit their structure"). *Rejected:* reuse `show_grid` — fewer buttons, but
  couples the two, and changes what an existing button does for every existing
  form. **Settled by the operator, 2026-10-15: separate.** (Spec R1–R2 name a
  "Show Grid" toggle and do not mention the existing one; the spec's wording
  now means this new button.)
- **D2 — Edit track lists at the token level.** *Why:* spec 056's parser skips
  what it cannot read, so parse-then-print loses a half-typed token; token edits
  cannot. It also keeps the developer's spelling (`1FR`, `100`, `Repeat(...)`
  casing) untouched outside the track being edited. *Rejected:* a structured
  `TrackList` with a printer — needs every token form to round-trip exactly.
- **D3 — Make implicit tracks explicit before editing.** *Why:* an implicit track
  is `Auto`, so writing it out changes nothing, and it removes a whole class of
  "the track I see has no text" cases. *Rejected:* refusing to edit such a grid
  — the grids that were built by placing children first are the common ones.
- **D4 — The pure operations live in `cobolt-forms`, not the IDE.** *Why:* they
  are unit-testable with no window, they are the same arithmetic the layout
  engine's `fr`/`%`/`px` rules define (so they belong beside them), and AC14's
  parity check is then a forms-crate test. *Rejected:* in `cobolt-ide` beside the
  canvas — the spec-094 precedent, but there the edits are about a control's own
  data, not about a layout the engine owns.
- **D5 — One `Cmd::GridEdit { cmds, removed }` wraps the batch.** `execute`
  applies `cmds` and, if `removed` is set, pushes it on the Restore history;
  `revert` reverts `cmds` and removes that entry. Restore is itself a
  `Cmd::GridEdit` with no `removed`. *Why:* R13 wants Restore separate from Undo
  *and* both working; if undoing a removal left its entry in the history,
  pressing Restore afterwards would insert the track a second time.
  *Rejected:* a history holding snapshots of the whole container and replacing
  it — Restore would then clobber unrelated edits made since, which R13 forbids.
- **D6 — Cells come from the solver.** *Why:* the designer must agree with the
  runtime about which cell an auto-placed child is in. *Rejected:* deriving the
  cell from each child's laid-out rectangle by point-in-track — wrong for a
  child with a non-`Stretch` alignment smaller than its cell, and for an empty
  span.
- **D7 — Mode precedence.** A press is, in order: a track-line grab band (when
  lines are shown) → the toolbar → (Show Grid on) the grid's cells → everything
  that exists today. *Why:* the order is the one the splitter and sidebar seams
  already use, so no existing gesture is reordered, only preceded.
- **D8 — Resize in pixels, re-express in units.** *Why:* the pointer works in
  pixels and the layout engine in units; going through pixels means every unit
  combination uses one rule and the neighbour really does change by `−d`.
  *Rejected:* per-unit special cases (px↔px, fr↔fr, …) — five code paths for a
  matrix that one path covers.
- **D9 — A grid child's handles map to tracks only with Show Grid off.** With it
  on the child cannot be selected at all (R6), so there are no handles to map.
- **D10 — The form's own grid is out of scope for this feature.** The layout
  output keys containers by control id; the form-level `LayoutMode = Grid`
  (spec 056 R55 also allows it) is not in it and its tracks live in form
  properties (`Cmd::SetFormProp`), not control ones. Supporting it adds a second
  property path and a second `ContainerGeom` source for a case PowerSpatial does
  not use (its grids are Panels). *Recommendation:* ship v1 for container
  controls, and take the form grid as a follow-up that reuses Layer 1 unchanged.
  **Settled by the operator, 2026-10-15: follow-up.**
- **D11 — Strings go in the IDE text tables, not `Tr` fields.** The spec says
  "new `Tr` fields in all six languages". The IDE has since moved its texts into
  `ui_text`/`ide_text_data.rs` (six languages per entry, guarded by
  `every_text_exists_in_six_languages` and the "no English reaches a widget"
  tests), and the existing "Toggle Grid" tip lives there. Same guarantee, the
  mechanism the code now uses.
- **D12 — The new module is `layout_grid_edit`.** `grid_edit` is spec 094's and
  means *DataGrid*; two files that differ by one word in a directory of 40 is how
  the wrong one gets edited.

## 5. Risks & mitigations

- **Default-on hides controls.** R2 + R6 mean that, on opening a form with Grid
  containers, a press inside one selects a cell, not a control. A developer who
  does not know why will think selection is broken. → The button is highlighted
  (R2) with a tooltip; the toolbar appears the moment a grid is touched; the
  sidebar's **Objects** list still selects any control by name. *Proposed, not in the
  spec:* a one-line hint on the toolbar's strip ("Grid editing — click the grid
  button to select controls"). Cheap, and the operator can veto it.
- **Track units and a scaled layout.** A card expanded by spec 090 lays its
  contents out at a zoom factor, so laid-out pixels are not designed pixels. →
  Line dragging and the cell-resize path are **off** for a container whose
  laid-out client differs from its designed client (`designed_client` vs
  `client` are both in `ContainerGeom`); the lines are still drawn.
- **Gaps.** With `Gap > 0` the "line" between two tracks is a band. → The line is
  drawn and grabbed at the band's centre; the drag moves both neighbours by the
  same `d`, so the band keeps its width.
- **`fr` tracks that cannot shrink.** A flexible track has an implicit minimum
  of its content (`MinMax(Auto, Nfr)`); a drag can ask for less than it allows.
  → The canvas shows what the engine actually lays out each frame, and the
  release commits the values written; the tests include a track held up by its
  content and say what happens (the line stops where the engine stops it).
- **Nested grids.** Innermost wins (R31); the outer grid stays reachable by a
  press outside the inner one's client, or from the tree.
- **A removal while another gesture is live, or Restore after the container was
  deleted or renamed.** → The history entry names the container by id; Restore
  is disabled when the container is gone, and a rename must rewrite the entry
  (a task: `Cmd::Rename` rewrites references on the form, not the designer's
  own session state).
- **Cross-session drift.** Editing a track list the developer also edits in the
  Properties pane: both write the same property, so there is nothing to
  reconcile — the next frame reads what is there.

## 6. Test strategy

*Layer 1 — `cobolt-forms`, table-driven, printing a result line each (GOLDEN
RULE #7):*
- `tracks::split/join` round-trip on a table of lists including `Repeat(…)`,
  `MinMax(…)`, mixed case and an unreadable token — which must come back
  unchanged (D2).
- `resize_pair` for every pair of units: `1fr 1fr` @ 25 % → `0.5fr 1.5fr`,
  `100px 200px` + 50 → `150px 150px` (AC10), `%`/`%`, `px`/`fr` with and without
  other `fr` tracks, `Auto` → `Npx`, an 8 px floor (AC11), `MinMax`/`Repeat`
  refused. Each case also *lays the result out* and asserts the pixel sizes
  asked for — so the unit arithmetic is checked against the engine, not against
  itself.
- `insert_track` / `remove_track` / `restore_track`: AC5, AC6, AC7, including a
  spanning child, an auto-placed child, an implicit track, the last track
  refused, and a `Repeat` axis refused (AC8).
- `merge` / `split_cell`: AC9, including the "exactly one control" rule counting
  a control in every cell its span covers.
- `Solved.cells`: explicit, auto-placed and spanning children match what
  `solve` puts in `rects`.

*Layer 2/3 — `cobolt-ide`, `designer_grid_layout_edit_tests.rs`, driving real
frames of the real `DesignerPanel::show` with pointer and key events (the spec
094 harness), on the committed PowerSpatial `main-form`:* AC1–AC4 and AC12–AC13
and AC15–AC16 as written; plus Undo/Redo/Restore ordering (D5) — undo a removal,
then Restore, and assert the track exists once; a press on a track line wins
over the control beneath, and a press off the band does not (R29); cursor icons.
Each gesture test asserts the **saved `.cfrm` bytes** it should change and no
others (R5, R24).

*Parity (AC14, R26):* after each operation, the canvas layout and the run-form
host's layout of the saved form give the same control rectangles — the existing
`layout::apply` harness (`Stringified` merge), applied to the edited form.

*Guards that exist and must stay green:* `cargo test -p cobolt-forms --features
render` (including `engine_reference_form_parity_static_vs_faces`);
`cargo test -p cobolt-ide` i18n/`ui_text` guards; the `egui-resize-guardian`
rule for the toolbar (fixed size, no `available_width`).

*Mutation checks:* each new gesture test is shown red by breaking the one thing
it claims to prove (the unit re-expression, the renumbering, the span shrink, the
undo/history sync) before it is trusted.

*Manual, for the operator (a headless run cannot judge look or feel):* open
PowerSpatial `main-form`; confirm the new button reads as highlighted and is not
confused with "Toggle Grid"; click a cell, add a column, merge two cells, drag a
line, remove and restore a row, Ctrl+Z each; turn the grid off, select a label,
see its grid, drag; Run Form and compare.

## 7. Steering compliance

- [ ] i18n: the new button's tip, every toolbar tip, the disabled reasons and the
      removal notice in six languages (D11)
- [x] Generated-code banner + regenerate-on-action contract preserved — grid
      edits are form properties; codegen does not read them for layout
- [ ] English dev guide updated ("Editing a grid on the canvas"); no translation
      exists to delete
- [x] Fix vs feature: **feature** → `features` branch, `z` bump, CHANGELOG entry
- [x] No "cobolt" in user-facing text; COBOL identifiers English
- [x] System KB: no control/property/method/event changed — no store rebuild
      unless `ColumnSpan`/`RowSpan` docs prove inaccurate when re-read
- [x] No self-resizing window: the toolbar is a fixed-size `Area`, laid out from
      its button count

## 8. For the operator — answered 2026-10-15 ("1 ok, 2 ok")

1. **D1 — the Show Grid button.** The designer already has a **Toggle Grid**
   button (the alignment dots; on by default). The plan adds a *separate* button
   for layout grids, with its own icon and tip. The alternative is to make the one
   existing button do both. *Recommendation: separate.* (If you want them
   joined, spec R1–R2 need an amendment, and R6 then applies to every form that
   has the dots on — which is every form.)
2. **D10 — the form's own grid.** v1 covers Grid *containers* (Panel, GroupBox,
   TabControl), which is what PowerSpatial uses. A form whose own `LayoutMode` is
   Grid keeps its tracks in the Properties pane for now. *Recommendation:
   follow-up, reusing Layer 1.*

Also flagged, with no answer needed unless you object: the optional hint line on
the toolbar (§5), and D11 (texts in the IDE text tables rather than `Tr`).
