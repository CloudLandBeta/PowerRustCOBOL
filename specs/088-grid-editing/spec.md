# Spec — Grid editing in the Form Designer

- **Status:** draft
- **Folder:** specs/088-grid-editing/
- **Author:** operator request, written by Claude   **Date:** 2026-10-05, amended 2026-10-07

## 1. Overview
A Grid container (`LayoutMode = Grid`, spec 056 R55–R56) is designed today
only through text: `GridColumns`, `GridRows`, and each child's `GridColumn`,
`GridRow`, `ColumnSpan` and `RowSpan` typed into the Properties pane. On the
canvas a selected grid shows read-only dashed track lines with their sizes
(056 R45). Nothing on the canvas edits the grid, so building a dashboard like
PowerSpatial's main form means switching back and forth between the canvas
and numbers.

This feature makes the grid directly editable on the canvas:
- a **Show Grid** toggle that switches the canvas between editing the grid and
  editing the controls;
- a floating, icon-only **grid toolbar** beside the selected grid, for adding,
  removing, restoring, merging and splitting;
- track lines that can be dragged;
- control resizing that, inside a grid, resizes the control's cell;
- selecting a control that sits in a grid shows **that grid**, with track
  lines the developer drags to size it, even while the grid is hidden
  (amendment A1).

## 2. Goals / Non-goals
- **Goals**
  - Edit a grid's structure on the canvas: add/remove rows and columns,
    restore a removed one, merge and split cells, resize tracks by dragging.
  - Two clear modes, chosen by one toolbar toggle: **grid visible** = editing
    the grid; **grid hidden** = editing the controls, where a control's size
    in a grid is its cell's size.
  - In either mode, the grid a selected control sits in is shown and can be
    resized by dragging, because on a responsive form that grid is how a
    control's size is set (A1).
  - Every edit is an ordinary property change: undoable, saved in the
    `.cfrm`, and identical at run time.
- **Non-goals**
  - No new run-time behaviour, no new layout model, no new property: spans
    (`ColumnSpan`/`RowSpan`) and every track form already exist in the
    engine (`cobolt-forms/src/layout/grid.rs`, `tracks.rs`). Run Form, child
    forms and the compiled binary are untouched.
  - Flex and Flow containers are not covered (they have no cells).
  - No grid editing at run time.
  - No visual editor for `Repeat(…)` / `MinMax(…)` track syntax; those
    tracks stay editable as text (R14).

## 3. User stories
- As a form designer, I want to add a column to a grid with one click, so
  that I do not have to retype `GridColumns` and renumber every child.
- As a form designer, I want to remove a row and have the other rows grow to
  fill the space, and to bring it back if I change my mind.
- As a form designer, I want to merge cells so that a card spans two columns,
  and split them again.
- As a form designer, I want to drag the line between two columns to resize
  them, as in a spreadsheet.
- As a form designer, I want to turn the grid off and simply resize a control,
  knowing that what I am really resizing is its cell.
- As a form designer on a responsive form, I want selecting a control to show
  me the grid it sits in and let me drag that grid's lines, because the
  control's own size is not mine to set — the grid's is.

## 4. Requirements (EARS)

### 4.1 The Show Grid toggle
- **R1 (ubiquitous):** The Form Designer's icon toolbar shall carry an
  icon-only **Show Grid** toggle button with a translated tooltip.
- **R2 (ubiquitous):** Show Grid shall be **on** by default, and while on it
  shall be painted in the toolbar's highlighted (active) state, as the Run
  Form button is when selected.
- **R3 (state):** While Show Grid is on, the canvas shall draw the track lines
  of **every** Grid container on the form, not only the selected one.
- **R4 (event):** When the developer toggles Show Grid, the canvas shall
  switch modes at once, without changing the form.
- **R5 (ubiquitous):** The toggle is a designer view setting. It shall not be
  saved in the `.cfrm` and shall not change any control or property.

### 4.2 Grid-editing mode (Show Grid on)
- **R6 (state):** While Show Grid is on, a pointer press inside a Grid
  container shall act on the grid (its cells and track lines), not on the
  controls placed in it. Those controls shall not be selectable, movable or
  resizable from the canvas.
- **R7 (state):** While Show Grid is on, the developer shall be able to select
  one cell by clicking it, and a rectangular range of cells by dragging across
  them or Shift-clicking a second cell. The selected cells shall be visibly
  highlighted.
- **R8 (state):** While Show Grid is on and a Grid container (or a cell in
  it) is selected, a floating **grid toolbar** shall appear beside the
  container, outside its bounds, without covering it, and clamped to the
  canvas.
- **R9 (ubiquitous):** The grid toolbar's buttons shall be icon-only, each
  with a translated tooltip, and disabled (not hidden) when the action does
  not apply to the current selection.

### 4.3 Grid toolbar actions
- **R10 (event):** **Add row / Add column.** When pressed, the system shall
  insert a track after the selected cell's row/column (or at the end when no
  cell is selected), sized like the track it follows. Every child whose
  `GridRow`/`GridColumn` lies after the insertion point shall be renumbered so
  that it keeps its cell; a child spanning across the insertion point shall
  have its span grown by one.
- **R11 (event):** **Remove row / Remove column.** When pressed with a cell
  selected, the system shall remove that cell's row/column from
  `GridRows`/`GridColumns`. The remaining tracks then share the container's
  space by their existing rules (so `fr` tracks grow to fill the gap). Children
  after it shall be renumbered; a child spanning across it shall have its span
  shrunk by one.
- **R12 (constraint):** Removing a track shall never delete a control. A child
  that sat **only** in the removed track shall keep existing and shall become
  auto-placed (its `GridColumn`/`GridRow` on that axis set to 0), and the
  canvas shall show a notice naming the affected controls. The last remaining
  row or column cannot be removed (button disabled).
- **R13 (event):** **Restore.** When pressed, the system shall put back the
  most recently removed row or column of that container: the same track
  definition, at the same position, with the placements and spans of the
  children it held. Restore shall be available for every removal made in the
  current designer session, most recent first, independently of unrelated
  edits made since. It is separate from Undo, and both shall work.
- **R14 (constraint):** Add, remove and restore shall act on the track list as
  written. Where the affected axis contains a `Repeat(…)` entry, those buttons
  shall be disabled for that axis, with a tooltip saying to edit the track list
  in the Properties pane.
- **R15 (event):** **Merge cells.** When pressed with a range of cells
  selected, the system shall make the control in that range span it: its
  `GridColumn`/`GridRow` set to the range's top-left cell, and its
  `ColumnSpan`/`RowSpan` set to the range's size.
- **R16 (constraint):** Merge shall be enabled only when the range contains
  **exactly one** control (counting a control as "in" every cell its span
  covers). With none, or with more than one, the button shall be disabled, and
  its tooltip shall say why.
- **R17 (event):** **Split cells.** When pressed with a cell selected whose
  control spans more than one cell, the system shall set that control's
  `ColumnSpan` and `RowSpan` to 1, leaving it in its top-left cell.

### 4.4 Dragging track lines
- **R18 (state):** While Show Grid is on, the inner lines between tracks of a
  Grid container shall be draggable, with a resize cursor on hover.
- **R19 (event):** When an inner line is dragged, the two tracks it separates
  shall change size in opposite directions, so the container's total is
  preserved, with the live result on the canvas during the drag. Release
  commits one undoable edit.
- **R20 (ubiquitous):** Each track keeps its own unit when resized: a `px`
  track gets a new pixel value, a `%` track a new percentage, and two `fr`
  tracks re-share their combined `fr` total in the new proportion. An `Auto`
  track that is resized becomes a `px` track of the dragged size. A `MinMax(…)`
  or `Repeat(…)` track cannot be dragged (its line shows no resize cursor).
- **R21 (constraint):** No track shall be dragged below a minimum of 8 px on
  the canvas.

### 4.5 Control-editing mode (Show Grid off)
- **R22 (state):** While Show Grid is off, controls inside Grid containers
  shall be selectable as they are today.
- **R23 (event):** When the developer resizes a control placed in a Grid
  container, the system shall resize the **tracks** of its cell instead of the
  control: dragging the control's right (left) edge moves the line after
  (before) its last (first) column, and likewise the bottom/top edges for rows.
  For a control spanning several tracks, the outer line of its span moves.
  Track units follow R20, and lines on the container's outer border are not
  moved.
- **R24 (constraint):** A control in a Grid container shall never be given an
  x/y/width/height of its own by a canvas gesture. Its size is always its
  cell's.

### 4.5a Selecting a control shows its grid (amendment A1)

> *Operator, 2026-10-07:* "When the layout is responsive and the controls are
> placed in a grid, since I cannot change directly the size of the controls,
> selecting a control should actually make the grid visible where it sits and
> the grid should be resizable by the user using drag and drop."

- **R27 (event):** When the developer selects a control that is placed in a
  Grid container — on a form that lays out (spec 056), with Show Grid **off**
  — the canvas shall draw that container's track lines (and only that
  container's, among the grids not otherwise shown), with the selected
  control's cell highlighted, for as long as the control stays selected.
- **R28 (state):** While R27 shows a grid, that grid's **inner** track lines
  shall be draggable with a resize cursor on hover, exactly as R18–R21 define
  (opposite-direction resize, units kept, `MinMax`/`Repeat` tracks fixed,
  8 px minimum, one undoable edit per release), and the control stays
  selected throughout.
- **R29 (constraint):** The grid lines R27 shows take the press over the
  control beneath them only within the lines' own grab band; elsewhere the
  selected control keeps its ordinary gestures (it can be moved to another
  cell, and its edges still resize its cell's tracks per R23).
- **R30 (event):** When the selection moves to a control in another grid, or to
  a control not in a grid, or is cleared, the grid R27 showed shall be hidden
  again — unless Show Grid is on, in which case R3 already shows every grid.
- **R31 (ubiquitous):** For a control in a grid nested in another grid, R27
  shows the **innermost** grid — the one whose cell holds the control.

### 4.6 General
- **R25 (ubiquitous):** Every edit this spec defines shall be an ordinary
  property change through the designer's command system: one undo step per
  gesture, marking the form dirty, saved to the `.cfrm`.
- **R26 (ubiquitous):** The canvas preview after any grid edit shall equal
  what Run Form shows for the same `.cfrm`: the edit writes properties, and
  the one layout engine places them.

## 5. Acceptance criteria
- [ ] **AC1 (R1, R2, R5)** — A fresh designer window shows the Show Grid button
  highlighted. Toggling it changes no byte of the saved `.cfrm`.
- [ ] **AC2 (R3)** — On PowerSpatial's `main-form`, with Show Grid on and
  nothing selected, the track lines of every Grid container are drawn.
- [ ] **AC3 (R6)** — With Show Grid on, clicking `Lbl-AC-Sub` inside `Crd-AC`
  selects a cell of `Crd-AC`, not the label, and dragging cannot move it.
- [ ] **AC4 (R7, R8, R9)** — Selecting a cell shows the floating toolbar beside
  the grid. Every button has a tooltip in all six languages, and Merge is
  disabled for a single empty cell.
- [ ] **AC5 (R10)** — On a 2×2 grid `1fr 1fr` / `1fr 1fr` with a child at
  (2, 2), Add column after column 1 gives `1fr 1fr 1fr`, and the child is at
  (3, 2), in the same visual cell as before.
- [ ] **AC6 (R11, R12)** — Removing row 1 of a 2-row grid holding a child only
  in row 1: `GridRows` loses that track, the child survives with `GridRow = 0`,
  a notice names it, and the remaining row fills the container.
- [ ] **AC7 (R13)** — After the AC6 removal and an unrelated edit elsewhere,
  Restore puts the row and the child's placement back exactly, and the
  unrelated edit remains.
- [ ] **AC8 (R14)** — On a grid whose `GridColumns` is `Repeat(3, 1fr)`, the
  column add/remove/restore buttons are disabled with the explanatory tooltip.
- [ ] **AC9 (R15, R16, R17)** — Selecting a 2×1 range holding one control and
  pressing Merge gives it `ColumnSpan = 2`. Split returns it to 1. A range
  holding two controls has Merge disabled.
- [ ] **AC10 (R19, R20)** — Dragging the line between `1fr 1fr` to the 25 %
  mark yields `0.5fr 1.5fr` (total 2 kept). Between `100px 200px`, a 50 px
  drag yields `150px 150px`. An `Auto` track dragged becomes `Npx`.
- [ ] **AC11 (R21)** — No drag can produce a track narrower than 8 px.
- [ ] **AC12 (R23, R24)** — With Show Grid off, widening `Lbl-AC-Sub` by its
  right edge changes `Crd-AC`'s `GridColumns`, and the label's own `w` in the
  `.cfrm` is unchanged.
- [ ] **AC13 (R25)** — Every gesture above is undone by one Ctrl+Z.
- [ ] **AC14 (R26)** — For each AC above, the canvas layout and `rcrun
  run-form`'s layout of the saved form produce the same control rects (the
  existing static-vs-run parity harness).
- [ ] **AC15 (R27, R30, R31)** — On PowerSpatial's `main-form`, with Show Grid
  off, selecting `Lbl-AC-Sub` draws the track lines of `Crd-AC` (the grid that
  holds it) and no other grid, with its cell highlighted; selecting a control
  outside any grid hides them.
- [ ] **AC16 (R28, R29)** — In that state, dragging the line between two of
  `Crd-AC`'s columns resizes those columns (units kept, per AC10) as one undo
  step, the label stays selected, and the label's own `w` in the `.cfrm` is
  unchanged; a press on the label away from the line still selects/moves it.

## 6. Constraints & steering check
- **i18n:** the Show Grid tooltip, every grid-toolbar tooltip, the disabled
  reasons and the "controls became auto-placed" notice are new `Tr` fields,
  in all six languages.
- **Generated code / regenerate contract:** none. Grid edits are form
  properties, which codegen does not read for layout.
- **Runtime / hosts:** none. Spans and all track forms are already in the one
  layout engine. The interpreter-binary-parity rule is satisfied trivially and
  is verified by AC14, not assumed.
- **System KB:** no control, property, method or event is added or changed.
  The `ColumnSpan`/`RowSpan` docs are checked for accuracy, and the KB is
  rebuilt only if they change.
- **Docs:** `docs/developers-guide-en.md` gets a "Editing a grid on the
  canvas" section (both modes, the toolbar, track dragging) with screenshot
  placeholders. Its five translations are deleted per GOLDEN RULE #8.
- **No self-resizing windows:** the floating toolbar has a fixed size computed
  from its icon count, never from available space.
- **Classification:** **feature** (a new IDE capability). It goes on the
  `features` line and bumps `z` only.

## 7. Open questions
- **Q1:** Should Show Grid on/off persist across IDE restarts (an IDE
  preference), or always start on? *Proposed: always start on (R2), as
  asked.*
- **Q2:** Restore history (R13) lives for the designer session. Should it
  survive closing and reopening the form? *Proposed: no.*
- **Q4 (A1):** Should the grid R27 shows also offer the floating grid toolbar
  (add/remove/merge), or only the draggable lines? *Proposed: only the lines —
  the toolbar belongs to grid-editing mode (Show Grid on), where controls
  cannot be selected by mistake.*
- **Q3:** Should a range selection containing no control offer Merge as
  "reserve an empty spanned area"? The model has no span without a control,
  so *proposed: no (R16)*.
