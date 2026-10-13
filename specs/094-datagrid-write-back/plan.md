# Plan — DataGrid: fake data in the Designer, write-back at run time

- **Status:** draft → approved
- **Spec:** ./spec.md (approved 2026-10-12, decisions D1–D5)   **Date:** 2026-10-12

## 1. Approach
Four workstreams, in this order. The first is what the operator can see working
(AC11) and needs nothing new below it.

**WS-A — the run-time wire, indexed first (R7–R19).** Everything stays in the
interpreter, so all three hosts get it (R21). A small new module,
`cobolt-runtime/src/grid_edit.rs`, holds the pending-edit state and the source
adapters; `interpreter.rs` gains three small call sites, not three hundred lines.

1. *Seeds.* `seeding.rs` (and the codegen `write_indexed_file_binding_seed`) add
   `_BindingWritable` (the binding's mode) and `_BindingKeyFields` for a
   `DataBindingDef`; a quick-bind grid (`populate_datasource_grids`) computes
   writability itself: `AllowCellEditing` on **and** the `.cidx` declares a
   primary key (D2). The grid's design property `AutoCommit` is already in the
   object registry like every property.
2. *Noting an edit.* In the `COBOL-WAIT-EVENT` arm that receives a UI event
   (`interpreter.rs` ≈11199), after `drain_input()` has folded in `EditedRow`,
   `EditedColumn`, `EditedValue`, `PreviousValue`, an `onCellEdited` for a
   writable grid is recorded as a `PendingEdit` on that grid.
3. *Settling it.* The generated event loop runs the handler paragraph and comes
   back to `COBOL-WAIT-EVENT`. **That return is "after the handler returns"**
   (R9) without any hook in the handler: at the **top** of the next
   `COBOL-WAIT-EVENT` the interpreter settles every grid's pending edits — with
   `AutoCommit` on it commits each one; with it off it leaves them pending (R10).
   A handler that restored `PreviousValue` (`SetCellValue`) has changed the cell,
   so the commit sees nothing to write.
4. *Writing.* One function, `commit_edit(grid, edit)`, reads the **current** cell
   from `Rows`, finds the source row **by key** (R17), writes, commits; on any
   error restores `PreviousValue` (a `StateUpdate` to the grid, the way
   `SetCellValue` does), sets `CommitStatus`, raises `onCommitError`.
5. *Identity.* The key is the key field(s)' cells in the edited row of `Rows`,
   read at commit time — so a COBOL `Sort`, `DeleteRow` or the operator's own
   sort cannot misdirect the write the way a stored row number would. An edit to
   a key column is refused (a REWRITE cannot change the key): `CommitStatus` =
   FILE STATUS 21 for indexed.
6. *Pending cells survive a refresh* (R14, D3): after `RefreshBinding()` loads
   `Rows`, the pending values are written over their cells again and
   `CommitStatus` is left non-zero.

**WS-B — the new surface (R7, R11–R13, R22).** `AutoCommit` (bool, default on) —
model default, Properties pane checkbox in *Edit DataGrid settings*, KB, hover
help; runtime-only `CommitStatus`; event `onCommitError` in the DataGrid event
list; methods `CommitEdit()` and `DiscardEdit()` — `CommitEdit` is
`BINDING-UPDATE` made real (§WS-A.4), `DiscardEdit` is `BINDING-MARK-CLEAN` plus
restoring each pending cell. Both are added to `cobolt_ast::methods`'
vocabulary so `DG::CommitEdit()` parses as a call, dispatched beside
`REFRESHBINDING` (`interpreter.rs` ≈16539). A pending cell is painted with a
marker from a runtime-only `_PendingCells` property the renderer reads (R12).

**WS-C — the other sources (R15).**
- *COBOL table:* find the table row by `key_fields`, assign through the
  interpreter's own store-into-item path so the PICTURE rules apply, exactly as
  a `MOVE` would.
- *SQL — settled by the T10 spike, in this feature.* What the spike found:
  `query_name` and `result_set_name` are **placeholders** the editor hard-codes
  (`"DEFAULT-QUERY"`, `"RESULT-SET"`, `properties.rs` ≈1005); the `SqlDatabase`
  control has no query or table property (`Driver`, `ConnectionString`,
  `SqlConnection`, `AutoConnect`, `ConnectionDataItem`, `ResultSetDataItem`,
  `Mode`, `Busy`, `TimeoutMs`); the program runs SQL itself with
  `Query`/`Execute` on the control, whose `Open` stores the connection handle in
  the object's `_Handle`, and `db_runtime`'s `exec(handle, text)` takes **text
  only**. So the model holds neither a SELECT to load from nor a table to update.
  The work: `Sql` gains `query` and `table_name` (R25, serde-defaulted); the
  binding editor gets two fields; the Guardian gets the writable-SQL rules; a
  seed `_BindingKind = Sql` with `_BindingSqlControl`, `_BindingSql`,
  `_BindingTable`; `refresh_binding` runs the query on the control's handle and
  fills `Rows` (a connection that is not open yet leaves `Rows` empty and does not
  fail — the program opens it, then calls `RefreshBinding()`); the update writes
  each value as an escaped literal. The UPDATE rides the connection's own
  transaction state: autocommit unless the program has issued `BEGIN`, in which
  case the program's `COMMIT` ends it — documented in the Guide.

**WS-D — the Designer (R1–R6).**
1. `refresh_data_binding_target_properties` (`app.rs` ≈20010, called each frame
   at `designer.rs` ≈9616) stops rewriting a `DataGrid` target's `Columns`,
   `DataSource`, `Rows`, `AdvancedGrid`. Those are seeded **once**, by the
   existing `ApplyDataBinding` command and by a new *Reset sample data* action,
   from the same functions (`data_binding_preview_rows`,
   `datagrid_advanced_for_binding`). The Chart and ControlArray branches keep
   their per-frame refresh — they are not part of this request.
2. A form that opens with a bound grid and no columns is seeded once, as an
   undoable load-time fix-up (R2).
3. `merge_datagrid_binding_column` keeps a title that differs from the field's
   display name (R5).
4. *Grid edit mode* (R3, R6). The canvas draws static faces (`render_faces`,
   `RenderMode::Static`) and takes no cell input. Double-clicking a grid enters
   the mode: the **selected grid alone** is drawn through the interactive
   renderer (`render_form`, `RenderMode::Interactive`) in a child `Ui` at the
   grid's rect, with `AllowCellEditing` forced on in a render-only copy, so the
   existing in-cell editor, column resize and reorder are the ones that run. The
   `prop_updates` it returns (`Rows`, `AdvancedGrid`) become `Cmd::SetProperty`
   commands. A small floating icon toolbar (the spec 088 pattern) adds and
   removes a column and a row. Esc, a click outside, or a double-click on the
   frame leaves the mode; the control's move and resize handles are hidden while
   it is on.

   **Amended by T13 (2026-10-12) — the canvas never drew the grid.** The static face
   `paint::draw_control` has for a DataGrid is the label `⊞ DataGrid (N cols)`; the
   header, rows, filter wells and gutter are painted only by the interactive arm in
   `render.rs`. So the interactive renderer hosts **every** grid on the canvas, not
   only the edited one: each is drawn right after the static pass (before the corner
   masks) in a child `Ui` at its rect — the edited one live, the others **display-only**
   (a disabled `Ui` at full opacity, input queue taken away while it draws), so the
   Designer's own select / move / resize / rubber band work over them. Fallback (overlay
   over `DataGridLayout`) was not needed. Cost: the grids are drawn after every other
   face, so a control above a grid in z-order that overlaps it is under it on the canvas
   (a separate pass per grid would repeat the layer backgrounds). **Fixed in 1.91.26:** the static pass
   now hands each grid back at its own place in the paint order (`render_faces_with`), so a grid
   is under what is above it, in its layer's order. The two gestures the
   renderer lacks — rename a title, drag a title — use a replica of the header
   arithmetic (`grid_edit::header_cells`), guarded by a test against the painted titles.

**WS-E — closing.** KB tables + `chunked.data`, Guide, six-language strings, the
`tests/cobol` program, the PowerDemo3 demo, version and changelog.

## 2. Affected crates / files
- `cobolt-runtime/src/grid_edit.rs` — **new**: `PendingEdit`, per-grid state,
  `commit_edit`, `discard_edit`, the indexed / table / SQL adapters.
- `cobolt-runtime/src/interpreter.rs` — the note and settle calls in
  `COBOL-WAIT-EVENT`; `CommitEdit` / `DiscardEdit` dispatch; `binding_update`
  delegating; the pending re-apply in `refresh_*_binding`.
- `cobolt-runtime/src/indexed_ide.rs` — `GridSession` gains `position_of_key`
  (find a row by key bytes) so no caller reaches into `rows()`.
- `cobolt-form-host/src/seeding.rs`, `cobolt-codegen/src/data_binding.rs` —
  `_BindingWritable`, `_BindingKeyFields`.
- `cobolt-forms/src/model.rs` — `AutoCommit` default, `onCommitError` in the
  DataGrid event list; `render.rs` — the pending-cell marker.
- `cobolt-ast/src/methods.rs` — `COMMITEDIT`, `DISCARDEDIT`.
- `cobolt-compiler/src/lib.rs` — property, event and method tables (KB);
  `assets/knowledge/chunked.data` regenerated.
- `cobolt-ide/src/app.rs`, `panels/designer.rs`, `panels/properties.rs`,
  `panels/data_grid_columns.rs`, `i18n.rs` (×6).
- `docs/developers-guide-en.md` (*DataGrid*, *Data binding*); its translations
  deleted. `docs/cobol-support-matrix-en.md`.
- `examples/PowerDemo3/forms/Data/datagrid-form.cfrm` — `AllowCellEditing` on.
- `tests/cobol/datagrid-write-back/` — the quantified program (Golden Rule 7).

## 3. Data / model changes
- New design property `AutoCommit` (bool, default true) on `DataGrid`; older
  forms load with the default and are unaffected (R8, R20).
- New runtime-only properties `CommitStatus`, `_PendingCells`; new seeds
  `_BindingWritable`, `_BindingKeyFields`. Nothing new in the `.cfrm` schema
  beyond the property; `DataBindingDef` is unchanged.

## 4. Key decisions & alternatives
- **Commit in the interpreter at the next `WAIT-EVENT`, not in generated COBOL.**
  Why: one place, three hosts, no second writer. *Rejected:* performing the
  generated `COBOL-DATA-BINDINGS-UPDATE` from every `onCellEdited` handler —
  the handler is the developer's, and generated code would write what a refusing
  handler has just put back.
- **Identity by key cells, not by row number.** Why: `EditedRow` is a position in
  `Rows`, and `Rows` is sorted, filtered and deleted from. *Rejected:* a stored
  row-to-record list — it goes stale on the first COBOL `Sort`.
- **A grid keeps a pending *list*; `CommitEdit()` and `DiscardEdit()` act on all
  of it.** Why: with `AutoCommit` off the operator edits several cells before the
  code decides. *Rejected:* one pending edit — a second edit would silently
  replace the first.
- **Open the file per commit** (as the load already does), not a held session.
  Why: the engines' locks (redb takes one exclusive lock per process) must not be
  held across the operator's thinking time.
- **Designer: the real interactive renderer for the selected grid.** Why: it is
  the editor the grid already has (R6), so column resize, reorder and the cell
  box behave at design time as at run time. *Rejected:* a hand-built overlay over
  `DataGridLayout` — a second editor to keep in step — and a modal grid editor,
  which is not "visually on the canvas".

## 5. Risks & mitigations
- **Hosting an interactive control on a static canvas** (z-order, clip, selection
  handles, undo): the largest unknown. *Mitigation:* T13 is a spike with a
  fallback — the same edits through the overlay, which still uses the in-cell
  editor's state.
- **Lock conflicts / a second writer** on the file. *Mitigation:* the engine's
  FILE STATUS becomes `CommitStatus` and the edit is restored; AC6 covers a
  vanished record.
- **PICTURE conversion.** Text that does not fit the leaf is refused by
  `encode_field_display`; the error text is the status.
- **SQL scope** (§1, WS-C). Contained by the spike and the second-step option.
- **The Designer's per-frame refresh was also a feature** (the preview followed
  `MOVE`s and `VALUE`s in the code). D1 retires it for grids; the one-shot seed
  uses the same function so the first look is unchanged.

## 6. Test strategy
- `cobolt-runtime/tests/test_grid_write_back.rs` — an indexed fixture, through the
  interpreter's real event loop: auto-commit on; a refusing handler; off + confirm;
  off + discard; PICTURE too long; key column edited; record deleted between load
  and edit; sorted `Rows` write the right record; `RefreshBinding` with a
  pending edit. COBOL-table and SQLite (in-memory) variants. Each prints its
  cases.
- `cobolt-forms` — `AutoCommit` default and the event list; the pending marker.
- `cobolt-ide` — Designer tests of AC1 (seed once, edits undo, title survives,
  the bound grid draws its own rows).
- Three-host parity (AC8): the same event through the interpreter built by
  `rcrun run-form`, the child-form host and `run_form_app`.
- `tests/cobol/datagrid-write-back/` — Golden Rule 7 block.
- Manual: PowerDemo3 Datagrid demo with editing on; edit "Actor Caption", press
  "Load data", see it persist (AC11). *Never drive the IDE window* — through
  `run_form` and the Run Form process only; whether the grid edit mode *looks*
  right is the operator's.

## 7. Steering compliance
- [ ] i18n: the checkbox, the quick-bind note — all six languages
- [ ] Generated-code banner + regenerate-on-action contract preserved (codegen
  gains seeds only)
- [ ] English Developer's Guide updated; translations deleted
- [ ] System KB tables + `chunked.data` regenerated in the same change
- [ ] Three interpreter hosts (R21)
- [ ] Fix vs feature: **feature** → `features-datagrid-write-back`, `z` bump +
  changelog; PowerDemo3's one-property change rides in the same feature
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
