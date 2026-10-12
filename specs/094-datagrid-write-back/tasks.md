# Tasks — DataGrid: fake data in the Designer, write-back at run time

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-10-12

Ordered, small, independently verifiable. Each names the files, the requirements
and how to check it. Work package 1 is what the operator can try first (AC11).
Branch: `features-datagrid-write-back`. Sweeps are `--no-fail-fast`, every
`test result:` line read (CLAUDE.md).

## WP1 — the run-time wire, indexed
- [ ] **T1 — Seeds** (R8, R16)
  - Files: `cobolt-form-host/src/seeding.rs`, `cobolt-codegen/src/data_binding.rs`,
    `cobolt-runtime/src/interpreter.rs` (`populate_datasource_grids`)
  - Do: seed `_BindingWritable` (from the binding's mode) and `_BindingKeyFields`;
    a quick-bind grid is writable when `AllowCellEditing` is on and the `.cidx`
    declares a primary key.
  - Verify: unit tests for each combination (read-only binding, writable binding,
    quick-bind with and without a key, editing off) — only the writable ones seed true.
- [ ] **T2 — Find a row by key** (R17)
  - Files: `cobolt-runtime/src/indexed_ide.rs`, new `cobolt-runtime/src/grid_edit.rs`
  - Do: `GridSession::position_of_key(&[u8])`; `grid_edit.rs` with `PendingEdit`
    and the per-grid pending list.
  - Verify: `cargo test -p cobolt-runtime` — the key of a row is found after the rows
    were reordered and after a record was deleted.
- [ ] **T3 — Note and settle** (R9, R10)
  - Files: `cobolt-runtime/src/interpreter.rs` (`COBOL-WAIT-EVENT`)
  - Do: record an `onCellEdited` for a writable grid as pending; at the top of the
    next `COBOL-WAIT-EVENT`, commit each pending edit when `AutoCommit` is on, leave
    it when off.
  - Verify: a runtime test with a form program whose handler refuses an edit
    (`SetCellValue(PreviousValue)`) writes nothing; one that does not, writes it.
- [ ] **T4 — The indexed write** (R13, R15, R17, R18)
  - Files: `grid_edit.rs`, `cobolt-indexed` (`encode_field_display` is used, not
    changed)
  - Do: read the current cell and the key cells from `Rows`; locate by key; encode;
    `select_row` + `rewrite_row` + `commit`; on error `rollback`, restore
    `PreviousValue`, set `CommitStatus`, raise `onCommitError`. Refuse a key-column
    edit (FILE STATUS 21) and a column with no single source field.
  - Verify: AC2 and AC6 — file contents read back after each case.
- [ ] **T5 — The surface** (R7, R11, R12, R22 groundwork)
  - Files: `cobolt-forms/src/model.rs`, `cobolt-forms/src/render.rs`,
    `cobolt-ast/src/methods.rs`, `cobolt-runtime/src/interpreter.rs`,
    `cobolt-ide/src/panels/properties.rs`
  - Do: `AutoCommit` (default true) and its checkbox beside *Allow cell editing*;
    `CommitStatus`; `onCommitError` in the DataGrid event list; `CommitEdit()` /
    `DiscardEdit()` (vocabulary + dispatch beside `REFRESHBINDING`); the pending-cell
    marker from `_PendingCells`.
  - Verify: `cargo test -p cobolt-forms --features render`; `DG::CommitEdit()` parses
    as a call; AC3 (off + confirm, off + discard).
- [ ] **T6 — A refresh keeps what is pending** (R14, D3)
  - Files: `interpreter.rs` (`refresh_indexed_file_binding`, `refresh_datagrid_binding`)
  - Do: re-apply pending values over the loaded `Rows`; leave `CommitStatus` non-zero.
  - Verify: a test: edit with `AutoCommit` off, `RefreshBinding()`, the cell still shows
    the edit and `CommitStatus` is set.
- [ ] **T7 — Runtime tests and the three hosts** (AC2, AC3, AC6, AC7, AC8)
  - Files: `cobolt-runtime/tests/test_grid_write_back.rs`, host parity tests
  - Do: the cases of plan §6; the same edit through the interpreter each host builds.
  - Verify: all green; the existing `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed`
    and the binding tests unchanged.
- [ ] **T8 — PowerDemo3** (AC11)
  - Files: `examples/PowerDemo3/forms/Data/datagrid-form.cfrm`
  - Do: `AllowCellEditing` on for `DataGrid-1`; regenerate; Check.
  - Verify: with `run_form` — set a cell, press "Load data", read the cell back; the
    value is in `actors.idx` (restore the file afterwards).

## WP2 — COBOL table
- [ ] **T9 — Table write-back** (R15, AC4)
  - Files: `grid_edit.rs`, `interpreter.rs`
  - Do: find the table row by `key_fields`; assign through the interpreter's own
    store-into-item path (PICTURE rules); `CommitEdit` / `DiscardEdit` for a table.
  - Verify: AC4 — the WS cell holds the new value after confirm, the old after discard.

## WP3 — SQL
- [x] **T10 — Spike: what the SQL source is** (R15, R25) — **done 2026-10-12**
  - Finding: **in this feature**, with two model additions. `query_name` /
    `result_set_name` are editor placeholders and mean nothing; the `SqlDatabase`
    control carries no SELECT or table; the connection is reached by
    `obj_get(source_control_id, "_Handle")` and `DbRegistry::exec(handle, text)`
    (text only; `esql::Session` has parameters but belongs to embedded SQL and has
    no route from the control). So `Sql` gains `query` + `table_name` (R25), and the
    update writes escaped literals. Sizing: model + editor + Guardian (small), load
    path (small), update (medium). Plan §WS-C amended.
- [ ] **T11 — The SQL source's model, and loading it** (R15, R25)
  - Files: `cobolt-forms/src/model.rs` (`query`, `table_name`),
    `cobolt-ide/src/panels/properties.rs` (two editor fields, ×6 strings),
    `cobolt-project-tools/src/binding_guardian.rs`, `seeding.rs`,
    `cobolt-codegen/src/data_binding.rs`, `interpreter.rs` (`refresh_binding`)
  - Do: the two fields (serde-defaulted); the editor inputs; the Guardian rules; the
    `_BindingKind = Sql` seeds; run the query on the control's handle and fill `Rows`
    (an unopened connection leaves `Rows` empty, no fault).
  - Verify: a grid loads from the in-memory SQLite connection the SQL tests use; a form
    saved before the change still loads; the Guardian blocks a writable binding with no
    table or key.
- [ ] **T12 — SQL write** (R15, R25, AC5)
  - Files: `grid_edit.rs`, `db_runtime.rs` (a `kind(handle)` accessor)
  - Do: `UPDATE <table> SET <field> = <literal> WHERE <key> = <literal>` with escaped
    literals and validated identifiers; on a failure restore the cell and set
    `CommitStatus` to the driver's message.
  - Verify: AC5 — an edit persists; a value with a quote, a backslash and a NUL-free
    non-ASCII text round-trips; a failing constraint restores the cell.

## WP4 — the Designer
- [x] **T13 — Spike: an interactive grid on the static canvas** (R3, R6) — **done 2026-10-12**
  - Files: `cobolt-ide/src/panels/designer.rs` (read; a scratch test)
  - Do: draw the selected grid through `render_form` (`RenderMode::Interactive`) in a
    child `Ui` at its rect and capture `prop_updates`; check z-order, clipping under a
    rounded Panel, and selection handles.
  - Verify: the note says "works" or names the fallback (overlay over
    `DataGridLayout` driving the in-cell editor's state); the plan is amended.
  - **Finding — works, no fallback, and it is bigger than the spec assumed.**
    1. *The premise was wrong.* The canvas never drew the grid at all: `render_faces`
       is static and reaches `paint::draw_control`, whose `CT::DataGrid` arm is the
       placeholder `⊞ DataGrid (N cols)` — the real header, rows, filter wells and
       gutter exist only in the interactive arm of `render.rs`. So R1 ("the Designer
       draws the grid's own Columns / Rows / AdvancedGrid") needed the real grid drawn
       on the canvas **always**, not only in edit mode (operator's screenshots,
       2026-10-12). Chosen route: **every DataGrid is drawn by the shared interactive
       renderer** (`render_form_scoped`, `RenderMode::Interactive`) in a child `Ui` at
       its rect (`panels/grid_edit.rs::draw_grids`); no second painter, no edit to
       `cobolt-forms`.
    2. *Display-only for every grid but the edited one:* the child `Ui` is
       `disable()`d — so no widget of the grid (cells, header, filter wells, edges,
       scrollbar, focus) senses a click or drag and the Designer's select / move /
       resize / rubber band work over it — then `set_opacity(1.0)`, because `disable`
       multiplies the painter's opacity (verified: without it every fill halves) —
       and the frame's input queue and scroll delta are taken away while it draws and
       handed back after, so the grid reads no key or wheel notch (it would consume the
       wheel and the arrows whenever the pointer is over it). Tests prove each.
    3. *Edit mode* draws the one grid live in the same place (own id scope), with
       `AllowCellEditing` forced on and `AllowSorting` off in a render-only copy; the
       renderer's `prop_updates` (whitelisted to `Rows`, `AdvancedGrid`, `RowHeight`,
       `RowHeightOverrides`, `ColumnFilters` — never `EditedValue` & co.) become
       `Cmd::SetProperty`; a drag amends its own step, so a whole column drag is ONE
       undo step. Cell editor, column-edge resize and the ‹ › reorder arrows are the
       grid's own (R6).
    4. *z-order — the cost of a separate pass.* The static pass draws every face in
       z-order but cannot draw a grid, so the grids (in the engine's order among
       themselves) are drawn right after it, **before** the corner-notch masks (the
       run renderer's order: a grid does not clip itself at a rounded corner, the mask
       does). A control that is ABOVE a grid in z-order and overlaps it is therefore
       under the grid on the canvas. Splitting the static pass around each grid was
       rejected: with layers, every `render_faces` call paints all the remaining layer
       backgrounds at its end. PowerDemo3's Datagrid form is unaffected (its Labels and
       Buttons do not overlap the grid).
    5. *Clipping:* the child `Ui` is clipped to the intersection of the ancestors'
       content rects (`containers::clip_rect`) and the grid is given the rounded
       parent's arc through the renderer's own `_ContainerClip` descriptor
       (`container_clip_geometry`), as a child of a rounded Panel is.
    6. *Selection handles* are not drawn in edit mode and their hit zones are off; the
       frame band (8 px outside the grid) takes the pointer, so a double-click there
       leaves and no gesture starts a move.
    7. *Double-click used to open the grid's handler;* on a DataGrid it now enters edit
       mode (the handler stays one click away in the Properties pane).
- [x] **T14 — Seed once, never again** (R1, R2, R5) — **done 2026-10-12**
  - Files: `cobolt-ide/src/app.rs` (`refresh_data_binding_target_properties`,
    `apply_data_binding_target_properties`, `merge_datagrid_binding_column`),
    `panels/designer.rs`
  - Do: the `DataGrid` branch stops running per frame; `ApplyDataBinding` and a new
    *Reset sample data* action seed `Columns`, `AdvancedGrid`, `Rows` once; a bound
    grid with no columns is seeded once on load; a title that differs from the
    field's display name is kept.
  - Verify: AC1 (first part) — the bound grid draws its own rows a frame later; one
    undo restores; a renamed title survives re-applying.
  - Done: `refresh_data_binding_target_properties` skips a `DataGrid` target (Chart and
    ControlArray unchanged); the grid seed lives only in `apply_data_binding_to_form`,
    reached by `ApplyDataBinding` (one undo restores the controls), by
    `DesignerPanel::reset_grid_sample_data` (the same command, for *Reset sample data*)
    and by `seed_bound_grids_without_columns` (on open, `load_form_from_path`; a grid
    with any column is left alone). `merge_datagrid_binding_column` keeps a non-empty
    title (R5 — decision: any title that is not empty is the developer's, since an
    empty one is a column the grid did not have), and `Columns` is written with each
    field's current title so a kept title still finds its data. Two existing app.rs
    tests that simulated "the save/run refresh" now re-apply the binding instead —
    the per-frame refresh no longer touches a grid, so that was the only case left.
- [x] **T15 — Grid edit mode** (R3, R4, R6) — **done 2026-10-12**
  - Files: `panels/designer.rs`, `panels/data_grid_columns.rs`
  - Do: double-click enters; the cell editor, column resize and reorder run; each
    `prop_update` becomes a `Cmd::SetProperty`; Esc / click outside / frame
    double-click leaves; move and resize handles hidden while on.
  - Verify: Designer tests — a cell edit, a width change and a title rename land in
    the properties and undo.
  - Done: new `panels/grid_edit.rs` (state, render-only copy, pure property edits,
    header geometry, toolbar) hosted by `DesignerPanel` (`enter_grid_edit`,
    `grid_edit_pointer`, `show_grid_edit`). Esc (the next one, when the first was the
    cell editor's — egui drops the focus before the frame runs, so the state is read as
    the last frame left it), a press away (the mode lives one more frame so a cell being
    typed keeps its text), or a double-click on the 8 px frame band leaves it. Handles,
    arrow-key nudge, Delete and the canvas gestures are off while it is on. The two
    gestures the renderer does not have — double-click a title to rename, press-and-drag
    a title to reorder — are hit-tested against `grid_edit::header_cells`, which
    replicates the renderer's header arithmetic through the same `DataGridLayout`
    (a test checks the painted titles fall inside those cells for five grid shapes:
    plain, caption row, row numbers, filters, tall rows). A rename also renames the
    `Columns` line when that line was matched by the old title, so the column keeps its
    data. Bound and unbound grids take the same path.
- [x] **T16 — Add and remove a column and a row** (R3) — **done 2026-10-12**
  - Files: `panels/designer.rs`, `i18n.rs`
  - Do: a floating icon toolbar in edit mode (spec 088's pattern), translated tooltips.
  - Verify: Designer tests; the form still saves and reloads with the new shape.
  - Done: six icon-only buttons in a fixed-size foreground `Area` above the frame band
    (below when there is no room, clamped to the canvas): add column, remove column,
    add row, remove row, *Reset sample data* (disabled — not hidden — on an unbound
    grid), done. Remove acts on the selected cell's column/row, else the last. New
    columns and rows carry fake data (`Text n`, `0`, a date). Six `Tr` fields
    (`dg_edit_*`) in all six languages, inserted after `grid_new_row`. Spec 088 is
    spec-only in this tree (no grid toolbar was ever implemented), so the pattern
    followed is its R8/R9 text. *Reset sample data* is reachable from this toolbar
    only (a decision: no Inspector button).
- [x] **T17 — Designer tests** (AC1) — **done 2026-10-12**
  - Files: `cobolt-ide` tests
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide` green.
  - Done: `panels/designer_grid_edit_094_tests.rs` (28 tests through real frames of
    `DesignerPanel::show`), `panels/grid_edit.rs` unit tests (7) and one in `app.rs`.
    Full sweep: 1602 passed, 1 failed (`docs_embed::…every_document_ships_in_every_language`,
    the expected one).

## WP5 — closing
- [ ] **T18 — System KB** (R22, AC9)
  - Files: `cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`
  - Do: `AutoCommit`, `CommitStatus`, `onCommitError`, `CommitEdit`, `DiscardEdit`; the
    completed `AllowCellEditing` text; regenerate with `build_chunked_kb`.
  - Verify: `prebuilt_chunked_kb_matches_the_published_documentation` green.
- [ ] **T19 — Guide and matrix** (R23)
  - Files: `docs/developers-guide-en.md`, `docs/cobol-support-matrix-en.md`
  - Do: *DataGrid* and *Data binding*, with a COBOL example for each mode; delete the
    Guide's translations (Golden Rule 8).
  - Verify: the English mentions each new name; `docs_embed` shows the intended red.
- [ ] **T20 — Strings in six languages** (R24)
  - Files: `cobolt-ide/src/i18n.rs`
  - Verify: `cargo test -p cobolt-ide i18n` — no empty translation.
- [ ] **T21 — The quantified COBOL program** (AC10)
  - Files: `tests/cobol/datagrid-write-back/`, a runner under `cobolt-runtime/tests`
  - Do: auto-commit on; off + confirm; off + discard; bad PICTURE; vanished record.
  - Verify: prints the cases and `TESTS RUN` / `TESTS FAILED` / `OVERALL RESULT : PASS`.
- [ ] **T22 — Finalize**
  - Do: bump `z`, a changelog entry, the full `--no-fail-fast` sweep.
  - Verify: every `test result:` line read; PowerDemo3 checks clean; AC1–AC11 ticked.

## Done criteria
Every acceptance criterion in spec.md is checked, tests pass, docs and KB updated,
and the change is committed on `features-datagrid-write-back` as feature commits
(do **not** merge or push unless the operator asks). The Designer's *look*
(AC1's grid edit mode) is confirmed by the operator, never by driving the window.
