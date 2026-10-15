# Tasks — Grid editing in the Form Designer

- **Status:** draft → in progress → done
- **Plan:** ./plan.md   **Date:** 2026-10-15

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

**How every task is worked:** the test is written first and seen **red**, then
the code makes it green; a gesture or arithmetic test is then shown red again by
breaking the one thing it claims to prove (mutation check). Every new test prints
a result line with the numbers it measured (GOLDEN RULE #7). Each task adds its
own strings to `crates/cobolt-ide/src/ide_text_data.rs` in all six languages (T18
checks the whole set). Work on `features`; build with the shared target dir
(`CARGO_TARGET_DIR=…/worktrees/fixes/target CARGO_INCREMENTAL=0`).

Layers (plan §1): **A** = `cobolt-forms`, pure (T1–T8); **B** = commands and
canvas host in `cobolt-ide` (T9–T17); **C** = text, docs, finalize (T18–T20).

## Layer A — the grid edit model (`cobolt-forms`, no egui)

- [x] **T1 — Track lists as tokens** (D2; groundwork for R10–R21)
  - Files: `crates/cobolt-forms/src/layout/tracks.rs`
  - Do: `split(text) -> Vec<String>` (top-level spaces; `MinMax(…)` and
    `Repeat(…)` stay one token), `join`, `kind_of(token) -> Plain(Track) | MinMax
    | Repeat | Unreadable`, `has_repeat(text)`, and the writers `fmt_px`,
    `fmt_fr`, `fmt_percent` (integers for px; at most three decimals, no trailing
    zeros, for `fr` and `%`).
  - Verify: `cargo test -p cobolt-forms --features render --lib tracks` — a table
    that round-trips `1fr 1fr`, `100px 200px`, `1FR  100`, `Repeat(3, 1fr)`,
    `MinMax(100px, 1fr) 2fr` and a list with an **unreadable token**, asserting
    `join(split(x)) == x` for each (the unreadable one unchanged); prints the
    case count.

- [x] **T2 — The solver reports each child's cell** (D6; R7, R12, R16)
  - Files: `crates/cobolt-forms/src/layout/grid.rs`, `layout/mod.rs`
  - Do: `grid::Solved.cells` — first row, first column, row span, column span,
    0-based, one per item in the order given — from the `place` result `solve`
    already computes; `ContainerGeom.cells: Vec<(String, Cell)>` filled where
    `geom.columns`/`rows` are (the designer's `tracks` path). No change to any
    rectangle.
  - Verify: `cargo test -p cobolt-forms --features render --lib layout` — cases
    for explicit cells, auto-placed children, a spanning child and an implicit
    row; each asserts the cell matches the rectangle `solve` returns; every
    existing layout test unchanged and green (proves nothing moved).

- [x] **T3 — Implicit tracks and Add row / Add column** (D3; R10; AC5)
  - Files: `crates/cobolt-forms/src/layout/grid_edit.rs` (new, re-exported from
    `layout/mod.rs`)
  - Do: the input shape (`GridView`: the container's two track texts, the
    laid-out track counts and sizes, each child's cell and its own
    `GridColumn`/`GridRow`/spans as written); `materialise_implicit` (append
    `Auto` tokens for laid-out tracks the text lacks); `insert_track(axis,
    after) -> Edits` — the new track sized like the one it follows, every child
    after it renumbered, a spanning child's span grown by one.
  - Verify: `cargo test -p cobolt-forms --features render --lib grid_edit` —
    **AC5** (2×2 `1fr 1fr`/`1fr 1fr`, child at (2,2): add column after 1 →
    `1fr 1fr 1fr`, child at (3,2), same visual cell — checked by laying both forms
    out), an end-of-list add, a spanning child, an implicit-track grid, an
    auto-placed child left auto.

- [x] **T4 — Remove row / Remove column** (R11, R12, R14; AC6, AC8)
  - Files: `layout/grid_edit.rs`
  - Do: `remove_track(axis, index) -> Result<(Edits, RemovedTrack, Vec<String>),
    Refusal>` — drops the token; renumbers later children; shrinks a spanning
    child; a child held **only** by the removed track keeps existing and goes to 0
    on that axis (the third value names it, for R12's notice); refuses the last
    track and an axis holding `Repeat(…)`.
  - Verify: `grid_edit` tests — **AC6** (child only in row 1 survives with
    `GridRow = 0`, named in the notice list, the remaining row fills the
    container when laid out), spanning shrink, last-track refusal, **AC8**
    (`Repeat(3, 1fr)` refused with the reason), and that no control is ever
    removed.

- [x] **T5 — Restore** (R13; AC7)
  - Files: `layout/grid_edit.rs`
  - Do: `restore_track(&RemovedTrack, current view) -> Edits` — the same token at
    the same index (clamped), later children renumbered as an add would, the held
    children given back their recorded placements and spans, and **nothing else
    touched**.
  - Verify: **AC7** — remove a row, make an unrelated edit to another control,
    restore: the track and the child's placement are back exactly and the
    unrelated edit remains; plus restore into a grid that has since gained a
    column.

- [x] **T6 — Merge and Split cells** (R15–R17; AC9)
  - Files: `layout/grid_edit.rs`
  - Do: `merge(range) -> Result<Edits, Refusal>` — enabled only with **exactly
    one** control in the range (a control counts in every cell its span covers);
    sets its cell to the range's top-left and its spans to the range's size.
    `split_cell(cell)` — `ColumnSpan`/`RowSpan` to 1, top-left kept. The refusal
    says *why* (none / more than one), for the tooltip.
  - Verify: **AC9** — a 2×1 range with one control merges to `ColumnSpan = 2`,
    split returns it to 1; a range with two controls and one with none are
    refused with their distinct reasons.

- [x] **T7 — Resize a pair of tracks** (D8; R19–R21; AC10, AC11)
  - Files: `layout/grid_edit.rs`
  - Do: `resize_pair(axis, line index, delta px) -> Result<Edits, Refusal>` — pixel
    arithmetic, 8 px floor, each of the two re-expressed in its own unit (`px`
    rounded; `%` of the client size; `Auto` → `px`; `fr` chosen so the solver
    gives that pixel size while the other `fr` tracks keep theirs); `MinMax` and
    `Repeat` refused; refused too when the container's laid-out client differs from
    its designed client (a zoomed card, plan §5).
  - Verify: **AC10** (`1fr 1fr` → `0.5fr 1.5fr` at the 25 % mark; `100px 200px` +
    50 → `150px 150px`; `Auto` → `Npx`), **AC11** (no drag below 8 px, both
    directions), `%`/`%`, `px`/`fr` with and without other `fr` tracks, a gap > 0.
    **Every case lays the written result out and asserts the pixel sizes it
    asked for**, so the arithmetic is checked against the engine, not itself.

- [x] **T8 — Parity: the canvas and Run Form agree** (R25, R26; AC14, forms half)
  - Files: `layout/grid_edit.rs` tests (and `layout/apply.rs` helpers if needed)
  - Do: for each operation in T3–T7, apply the edits to a form, then compare the
    layout input the designer builds with the run-form host's merge
    (`Stringified`, as `layout/apply.rs` already does) — same control
    rectangles.
  - Verify: `cargo test -p cobolt-forms --features render` green, including
    `engine_reference_form_parity_static_vs_faces`; prints "N operations, M
    controls, 0 rectangles differ".

## Layer B — commands and the canvas host (`cobolt-ide`)

- [x] **T9 — `Cmd::GridEdit` and the Restore history** (D5; R13, R25; AC13 part)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: `Cmd::GridEdit { cmds, removed }` handled in `execute`/`revert` (apply
    the batch, push the `RemovedTrack`; revert it, drop the entry); the history
    on the designer; Restore applies `restore_track` as a `GridEdit` with no
    `removed`; a control rename rewrites the entries.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide grid_layout` — remove →
    Undo → Restore leaves the track **once**; remove → Redo → history has it
    again; remove, unrelated edit, Restore keeps the unrelated edit; a rename
    follows. Written red against a version without the sync.

- [x] **T10 — View state and hit testing** (R6, R7, R31)
  - Files: `crates/cobolt-ide/src/panels/layout_grid_edit.rs` (new),
    `panels/mod.rs`
  - Do: `LayoutGridView` (toggle default on, cell selection, history, notice);
    `grid_at(point)` — the innermost Grid container whose client holds the
    point; `cell_at`, `line_at` (4 px grab band, centre of a gap), a cell range's
    rectangle. Pure functions over `canvas_layout`; the mode is *active* only per
    plan §1 (form lays out; no Preview, Visual Tab Order, Non-Visuals tab,
    DataGrid edit, Format Painter or placing tool).
  - Verify: unit tests on a fixture with a grid inside a grid: innermost wins,
    gap and outer-border cases, inactive in each excluded mode.

- [x] **T11 — The Show Grid button and the track-line painter** (R1–R5, R3; AC1, AC2)
  - Files: `designer.rs` (`draw_icon_toolbar`, `DesignerToolbarAction`,
    `paint_container_overlay` generalised), `layout_grid_edit.rs` (the icon),
    `ide_text_data.rs`
  - Do: a distinct painted icon (framed table, two inner lines) and tip; the
    toggle, on by default, highlighted while on, **not** saved; with it on,
    every Grid container's lines and sizes are painted (the R45 painter taken from
    "the selected one" to "every one").
  - Verify: AC1 — a fresh designer has the button highlighted; toggling changes
    no byte of the form's saved `.cfrm`; AC2 — on PowerSpatial `main-form`,
    nothing selected, every Grid container's lines are drawn (count asserted);
    the dotted "Toggle Grid" still behaves as before.

- [x] **T12 — Grid-editing mode: cells and ranges** (R6, R7, R8-selection; AC3)
  - Files: `designer.rs`, `layout_grid_edit.rs`, new test file
    `panels/designer_grid_layout_edit_tests.rs`
  - Do: with the mode active, a press inside a Grid container selects the
    **container** and one cell; Shift-click or a drag extends to a range; the
    cells are highlighted; the controls inside are not selectable, movable or
    resizable from the canvas.
  - Verify: frame-driven (spec 094 harness) — AC3: clicking `Lbl-AC-Sub` inside
    `Crd-AC` selects a cell of `Crd-AC`, not the label, and a drag cannot move
    it (the form's bytes unchanged); range by drag and by Shift-click.

- [x] **T13 — The floating toolbar and its actions** (R8, R9, R10–R17; AC4–AC9)
  - Files: `layout_grid_edit.rs`, `designer.rs`, `ide_text_data.rs`
  - Do: a foreground `Area`, fixed position and fixed size from the button count,
    beside the container, outside it, clamped to the canvas; icon-only buttons
    (add/remove row and column, Restore, Merge, Split), each with a tip in six
    languages, disabled — never hidden — when it does not apply, with the reason
    as the tip where it is a 088 rule; actions run T3–T6 through `Cmd::GridEdit`;
    the removal notice (R12) names the controls made auto-placed; the optional
    hint line (plan §5).
  - Verify: frame-driven AC4 (toolbar beside the grid, a tip in all six
    languages, Merge disabled for one empty cell), AC5–AC9 through the buttons,
    asserting the saved properties; toolbar rectangle never overlaps the
    container and never leaves the canvas (several container positions).

- [x] **T14 — Dragging a track line** (R18–R21, R25, R28, R29; AC10, AC11, AC16)
  - Files: `designer.rs` (`DragState::DraggingGridLine`, press priority,
    cursor), `layout_grid_edit.rs`
  - Do: a press in a line's grab band outranks what is beneath it (the
    splitter-division order) and **only** within the band; the resize cursor on
    hover; the two tracks written live by T7; one undo step on release (the
    `MovingSplitterDivision` precedent); no cursor and no drag for `MinMax`,
    `Repeat`, a zoomed container, or a line on the outer border.
  - Verify: frame-driven AC10/AC11 through real drags (units kept, ≥ 8 px, one
    Ctrl+Z each); a press on a control off the band still selects it (R29);
    cursor icons asserted.

- [x] **T15 — Selecting a control shows its grid** (A1: R27, R30, R31; AC15, AC16)
  - Files: `designer.rs`, `layout_grid_edit.rs`
  - Do: with the toggle off, selecting a control in a Grid container paints that
    container's lines (innermost) and highlights the control's cell, for as long
    as it stays selected; none of another grid's; hidden again when the selection
    moves or clears; the lines are draggable per T14 with the control staying
    selected.
  - Verify: AC15 on `main-form` (`Lbl-AC-Sub` → `Crd-AC`'s lines only; a control
    outside any grid hides them); AC16 (drag between two of `Crd-AC`'s columns,
    the label stays selected, its own `w` unchanged in the `.cfrm`).

- [x] **T16 — Resizing a grid child resizes its cell** (R22–R24; AC12)
  - Files: `designer.rs` (`DragState::ResizingGridCell`, the release path), `layout_grid_edit.rs`
  - Do: with the toggle off, a handle on a control in a Grid container moves the
    line before/after its span (a corner moves one line on each axis; the outer
    border's lines are not moved) through T7; the control's own `x/y/w/h` is never
    written; refused the same cases as T14.
  - Verify: **AC12** — widening `Lbl-AC-Sub` by its right edge changes
    `Crd-AC`'s `GridColumns`, and the label's own `w` in the saved `.cfrm` is
    unchanged; a control in a non-grid parent resizes exactly as before.

- [x] **T17 — Every gesture is one undo step** (R25; AC13)
  - Files: tests in `designer_grid_layout_edit_tests.rs`
  - Do: one table that performs each gesture (add/remove row and column,
    restore, merge, split, line drag, cell resize) and asserts a single Ctrl+Z
    returns the form byte-for-byte, and Ctrl+Y re-applies it.
  - Verify: the table's result line (gestures, undone exactly, redone exactly).

## Layer C — text, docs, finalize

- [x] **T18 — Texts and the System KB check** (spec §6; i18n)
  - Files: `ide_text_data.rs`; `cobolt-compiler/src/lib.rs` read-only unless wrong
  - Do: confirm every new string (button tip, toolbar tips, disabled reasons,
    notice, hint) is in all six languages, with the code names kept; re-read the
    `ColumnSpan`/`RowSpan` documentation against what the engine does and rebuild
    `assets/knowledge/chunked.data` **only** if it is wrong.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide ui_text` and
    `every_completion_description_exists_in_six_languages` unchanged by this
    feature; `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [x] **T19 — Developer's Guide** (spec §6; GOLDEN RULE #3)
  - Files: `docs/developers-guide-en.md`
  - Do: "Editing a grid on the canvas", after *Responsive examples in
    PowerDemo3*: the two modes and the button (and how it differs from *Toggle
    Grid*), the floating toolbar, dragging a line, resizing a control's cell,
    Restore, and the limits (`Repeat`/`MinMax`, a form's own grid) — COBOL and
    prose only, original wording; screenshot placeholders
    (`grid-edit-mode.png`, `grid-edit-toolbar.png`, `grid-edit-drag.png`) saying
    what to capture. No translation exists to delete.
  - Verify: the Guide's own tests (`docs_embed`) unchanged; read once as a
    PowerCOBOL/isCOBOL developer would.

- [x] **T20 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs` (`z` bump), `CHANGELOG.md`, this
    file (ticks), `spec.md` (acceptance criteria ticked with their proofs; status)
  - Do: sweeps on every crate touched, each `test result:` line read —
    `cobolt-forms --features render`, `cobolt-ide` (bin and tests; name what is
    red and why, as the last two entries did), plus `cobolt-form-host` and
    `cobolt-cli` to show the runtime is untouched (R26); the mutation checks
    listed in their tasks; the manual checklist of plan §6 handed to the operator.
  - Verify: every AC in `spec.md` ticked with its proof, or left unticked with
    the reason (AC14's run-form half is the host merge, not a spawned window).

## Acceptance criteria → tasks

AC1 → T11 · AC2 → T11 · AC3 → T12 · AC4 → T13 · AC5 → T3, T13 · AC6 → T4, T13 ·
AC7 → T5, T13 · AC8 → T4, T13 · AC9 → T6, T13 · AC10 → T7, T14 · AC11 → T7, T14 ·
AC12 → T16 · AC13 → T9, T17 · AC14 → T8, T20 · AC15 → T15 · AC16 → T14, T15.

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and the
change is a single **feature** commit on `features` (`z` bump). Do **not**
commit or push unless the operator asks.
