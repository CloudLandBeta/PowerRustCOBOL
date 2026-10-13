# Plan — DataGrid: writable from the Designer, `Save()` in one transaction, cells that take a click

- **Status:** draft → approved
- **Spec:** ./spec.md (approved by the operator on 2026-10-12, decisions D1–D3)   **Date:** 2026-10-13
- **Branch:** `features-datagrid-save` (worktree `.claude/worktrees/datagrid-save`). It sits at 1.91.17 and
  `main` is at 1.91.20, so the first step of `/implement` is `git merge --ff-only main` (Golden Rule 5; the
  spec commit is already an ancestor of `main`). Four commits, one `z` bump each (§1, *Milestones*).

## 0. What reading the code changed

The spec was written from a read of 094. This plan re-read the code; where it differs, **the code wins** and the
plan says so. The spec file is not edited.

| # | Spec says | Code says | Effect on the plan |
|---|-----------|-----------|--------------------|
| F1 | §2: a Dropdown column "already has configuration … saved in the binding" | `DropdownConfig` lives only in the IDE (`properties.rs` ≈1471). `binding_fields()` writes only `edit_control`; reopening a binding rebuilds every row with `DropdownConfig::empty()` (≈889). `DataBindingDef`, `BindingField` and `DataGridColumn` have no lookup field, and **nothing at run time resolves a Dropdown's items.** | The Dropdown cell (R15) needs a *persisted* config, a resolver and a popup that the cell can use. It is the largest piece of the spec and is its own milestone (M4) — the escape hatch spec §10 already names. |
| F2 | R3/Q2: a table row is found by key cells | `refresh_datagrid_binding` (≈12578) **skips blank table rows**, so grid row *n* is not table occurrence *n*. | Occurrence identity needs its own list, not arithmetic (WS-A5). |
| F3 | R6/R7: a batch can wait pending | 094's `PendingEdit` is `{row, col, …}` — a **position** in `Rows`. A COBOL `Sort`/`DeleteRow` between the edit and `Save()` leaves the position pointing at another row. | Pending edits carry their row's identity from the moment they are noted (WS-A1). |
| F4 | AC3: "the statements the run sent" | `DbRegistry::exec` (`db_runtime.rs:566`) records nothing and cannot say whether a transaction is open. Only the *EXEC SQL* backends can (`esql/backend/*::in_transaction`). | A transaction-state query and a statement recorder are added to the registry (WS-A4). |
| F5 | R23: three hosts | Four sites build the interpreter: `rcrun run-form` (`form_gui.rs:535`), the child-form host (`host.rs:5084`), the compiled binary's `run_form_app` (`cobolt-compiler/src/lib.rs:4054`) **and `rcrun run-form-ipc`** (`cobolt-cli/src/main.rs:257`). | All four are listed in the parity check. The change lives in the interpreter, so no host edit is expected. |
| F6 | AC12: "a program under `tests/cobol`" | `tests/cobol/datagrid-write-back/` does not exist: 094 printed its quantified block from `write_back_report` in `test_grid_write_back.rs`, because a console COBOL program has no event loop to carry a grid. | Same here: a `save_report` block in Rust (Golden Rule 7's content, not its folder). **Needs the operator's yes — I7.** |
| F7 | R15: a Dropdown "opens a list" | The grid's text editor **already opens** on a double-click or F2 in a Checkbox/Dropdown column (`column_editable` excludes only Image and data-less columns, `render.rs` ≈10782). Cell hit-rects are unclipped and the last registered wins; the whole cell `interact` sits inside `SelectableText` (≈11609). | The two kinds leave the text editor, get hit-rects clipped to the visible body, and no longer depend on `SelectableText` (WS-D). |
| F8 | AC9: "equal on canvas, Preview and Run Form" | The corpus golden's *Canvas* surface is `render_faces`, which draws a DataGrid as a one-line label; the Designer draws grids through the interactive renderer (`grid_edit.rs` in the IDE, `draw_grids` → `render_form_scoped`). | The cell golden compares the three **interactive** entry points: `render_form` (Run), `render_form_with_chrome` (Preview), `render_form_scoped` (Designer). |
| F9 | R25 | `CommitEdit`/`DiscardEdit` are in `control_method_docs` but **not** in the compact table (`cobolt-compiler/src/lib.rs` ≈8788–8802). | The compact table gains all three (`Save`, `CommitEdit`, `DiscardEdit`). |

## 1. Approach

Five workstreams. Order follows spec §7; M1–M4 are the commits.

### WS-A — The unit of work (R5–R12, R19–R21), runtime only

Everything below is in the interpreter, so every host gets it. `interpreter/grid_edit.rs` (631 lines) keeps the
pending state, settle and the methods; the three writers move to a new sibling `interpreter/grid_unit.rs`
(planning, validation, the three backends), so neither file passes ~700 lines.

**A1 — A pending edit knows its row.** `PendingEdit` gains `id: RowId`, captured in `note_grid_edit`, read from
the grid at that moment:
`RowId::Cells(Vec<(field, text)>)` — the key cells (indexed primary key from the `.cidx`, `_BindingKeyFields`
for SQL and table) — or `RowId::Occurrence(n)` for a keyless COBOL table. One helper (`row_identity`) serves the
note and the write, so 094's key-cell rule (R17 of 094) has a single implementation. At write time `locate()`
tries `rows[edit.row]` first (the common case) and otherwise scans `Rows` for the id; no match → `23`, "the row
is no longer in the grid". `row`/`col` stay what the handler sees (`EditedRow`, `EditedColumn`).

**A2 — Plan, then apply.** `commit_unit(grid, door)` replaces `commit_grid_edits`/`commit_one`:

1. *Collect.* For every pending edit: `locate()`, read the cell's **current** text; skip it when it equals
   `previous` (a handler restored it = refused, 094 R9). Group the rest **by source row** (`RowChange { id,
   cells: Vec<(col, field, text)> }`), rows in ascending grid order so statements and rewrites are deterministic.
2. *Plan (no write).* Per backend, build the whole job and fail on the first problem: a key column edited
   (`21`, R12), a column with no field behind it (`91`), a value the PICTURE/number rules refuse (`92`), a row
   that is not in the source (`23`). Nothing has been touched when this fails.
3. *Apply.* Inside one transaction on the source (A3–A5). Any error rolls the unit back.
4. *Outcome* (A6).

Entry points — all three call `commit_unit`: `Save()`, `CommitEdit()` (same function; R11), and
`settle_grid_edits` for an `AutoCommit` grid (R10: the edits kept by that event are one unit).

**A3 — Indexed.** One `GridSession::open` for the whole unit; positions of all target records are found first
(keys never change, so positions stay valid), then `select_row` + `rewrite_row` per record with every changed
field of that record encoded by `encode_field_display`; then **one** `commit()`; on any failure `rollback()`.
`GridSession` gets a small `position_of_key` so `grid_unit` stops reaching into `rows()` (094's plan promised
it). `rewrite_row` reloads all rows after each rewrite, so a Save of *k* rows costs *k* scans; the report prints
the elapsed time, and it is optimised only if a measurement says so. All three engines make several rewrites one
unit (disk undo log, redb write transaction, in-memory journal — read in `indexed_disk.rs:2578`,
`indexed_redb.rs:1399`, `indexed.rs:894`).

**A4 — SQL.**
- *Knowing a transaction is open* (spec §10). `DbConn::in_transaction()`: SQLite asks the driver
  (`!is_autocommit()`, as the EXEC SQL backend does); PostgreSQL and MySQL keep a flag that `DbConn::exec` sets
  and clears from the leading keyword of each statement it runs (`BEGIN`/`START TRANSACTION` and
  `SET autocommit = 0|OFF` open; `COMMIT`/`END`/`ROLLBACK` — not `ROLLBACK TO` — and `SET autocommit = 1|ON`
  close; a `;`-separated string is read statement by statement outside quotes). That is how the EXEC SQL
  backends already keep `in_tx` for those two engines (`postgres.rs:367`, `mysql.rs:260`); neither driver
  exposes the server's flag (`postgres` 0.19 has no `transaction_status`; `mysql` keeps `status_flags` private).
  `DbRegistry::in_transaction(handle)` forwards it. Every statement on a `SqlDatabase` handle — `Execute`,
  `Query`, `COBOL-EXEC-SQL` — goes through `DbRegistry::exec`, so the flag sees them all.
- *Preconditions, in order:* handle open (`90`); the control's `Mode` is not `Async` (new code **`95`**, Q5);
  no transaction already open (new code **`94`**, R6). A refusal sets `CommitStatus`/`LastError`, returns the
  code and writes nothing — the program's transaction is untouched. (Codes `94`–`97` are free: the engine owns
  `90 92 93 99`, 094 owns `90 91 92 98`.)
- *Apply:* `BEGIN`; one `UPDATE t SET c1 = v1, c2 = v2 WHERE k = …` per changed row (values through the existing
  `sql_literal`/`is_plain_name`); an `Err` or **0 rows affected** → `ROLLBACK`, `23`; then `COMMIT`; a failing
  `COMMIT` → `ROLLBACK`, `90`.
- *Test hook:* `DbRegistry` can record the statements it ran (`#[doc(hidden)] record_statements`), so AC3 and AC5
  assert `BEGIN, UPDATE, UPDATE, COMMIT` and "no `BEGIN`" literally.

**A5 — COBOL table.** Plan = find every row, validate every value (field exists, not a key, numeric item gets a
number); apply = assign all, as `MOVE` would (R6, R21). Row identity is the key cells when the binding names
keys, otherwise the **occurrence** (R3/Q2): the interpreter publishes a runtime-only `_RowOccurrences` (one
table occurrence per `Rows` line, filled by `refresh_datagrid_binding`) and keeps it in step with `Sort`,
`DeleteRow`, `AddRow`, `ClearRows` through the helper that already keeps per-row heights in step
(`remap_row_heights`, `interpreter.rs` ≈14018). A row added by `AddRow` has no occurrence and is refused with
`23`. With `AutoCommit` on a table edit stores itself with no code (R19, R20 is the `AutoCommit` off path).

**A6 — Outcome.**
- *Success.* Pending edits of the unit cleared; the cells show what the source holds (094: `5` in `9(4)` is
  `0005`); `CommitStatus` `00`, `LastError` empty, `_FailedCells` empty; `SavedRows` = source rows written;
  `onSaved` queued **once** when at least one row was written. An empty unit does not open a transaction, answers
  `00`, `SavedRows` 0, raises nothing (R9).
- *Failure, `AutoCommit` off* (`Save()`/`CommitEdit()`, R7/Q6): the unit is rolled back; **every edit stays
  pending and the cells keep the operator's values**; the failing cell is published in a new runtime-only
  `_FailedCells` (same `row,column;…` format as `_PendingCells`, painted as a red mark); `CommitStatus`,
  `LastError`, `EditedRow/Column/Value/PreviousValue` name the failing edit; `onCommitError` is raised once.
- *Failure, `AutoCommit` on* (the settle after a handler): **094's behaviour is kept** — the unit is rolled back,
  each edit's cell goes back to `PreviousValue` and nothing stays pending, then the same status, error fields
  and one `onCommitError`. Reason and trade-off in §4 (D5).
- *Refusals* (`91`, `94`, `95`): status and error only; no `onCommitError` (no row failed), edits stay pending.
- `reapply_pending_edits` (a refresh with edits pending, `98`) re-locates each edit by id, so a reload that
  reorders rows lands the values on the right rows.

### WS-B — The surface (R5, R8, R25)

- Method `Save()` → `String`: `"SAVE"` in `cobolt_ast::methods::is_known_method` (`methods.rs` ≈160, beside
  `COMMITEDIT`), a `"SAVE" if <class is DataGrid>` arm beside `COMMITEDIT` (`interpreter.rs` ≈16576) calling the
  same `save_method` as `CommitEdit`. The guard matters: `exec_method`'s default arm treats an unknown name as a
  property accessor, and a name added to the vocabulary stops being readable as a property on any control (the
  comment at ≈17487). The scan test `every_dispatched_control_method_is_spellable_inline` (≈23568) fails until
  both halves exist.
- Event `onSaved`: `supported_events` DataGrid arm (`model.rs` ≈3672). Runtime properties `SavedRows`,
  `_FailedCells`, `_RowOccurrences` (and, M4, `_ColumnItems`) join `DATA_GRID` (`model.rs` ≈1504), next to
  `_PendingCells`. Raised with `queue_control_event(grid, "onSaved")` after `SavedRows` is set.
- Codegen needs nothing: events come from the form's own bindings (`write_event_loop`), and no new seed is added
  (`_BindingWritable`, `_BindingKeyFields` already travel; `_RowOccurrences` and `_FailedCells` are produced at
  run time).

### WS-C — The Designer (R1–R4)

All in `cobolt-ide`; every step reachable from the headless tests.

1. **Mode round-trips.** `BindingEditorState` gains `mode: BindingMode` (`properties.rs` ≈711–740);
   `from_existing` reads `binding.mode` (≈829); `to_binding` sets it after `DataBindingDef::new` (≈983).
   Today re-opening a Writable binding and pressing Apply **silently turns it read-only** — R1's "keeps the mode
   it was saved with" is a bug fix in the same stroke. While there: `from_existing` also keeps the descriptor's
   own `key_field`/`key_fields` that no field row flags (it reads only the flags today, so a hand-written
   writable binding would lose its keys on Apply).
2. **The switch and what the grid still needs (R1, R2).** A new `show_writable_section` after the fields table in
   the Indexed, SQL and COBOL-table arms of `show_binding_editor` (≈5940–5956); hidden for REST (the editor always
   writes `update: None`, which the Guardian blocks when writable). Off by default. A pure
   `writable_needs(editor, ctrl) -> Vec<Need>` (so it is unit-testable) returns: `KeyField` (Indexed, SQL: no row
   ticked *Key*), `SqlTable` (SQL: no table), `CellEditingOff` (target is a DataGrid and `AllowCellEditing` is
   not on). For a COBOL table with no key it states "rows are found by their place in the table" rather than a
   need. `CellEditingOff` has a one-click fix: `action.set_props.push((ctrl.id, "AllowCellEditing",
   PropValue::Bool(true)))` — the mechanism `bool_row` uses; it is its own undo step, before Apply (the editor
   cannot make it part of `ApplyDataBinding`'s snapshot). A missing key has no fix button, only the pointer to
   the *Key* column. The editor does **not** call the Guardian (it only gates Save/Run/Check/Build today).
3. **Guardian** (`binding_guardian.rs` ≈598). `missing-row-identity` stops blocking a writable
   `CobolTable` source with no key; every other source keeps it. Together with A5 this is what makes a keyless
   table writable end to end (a Guardian change alone is not enough — the runtime used to refuse an empty key
   list, `write_table`).
4. **Strings (R4).** About eight `Tr` fields (switch label, one-line explanation, the three needs, the fix
   button, the by-occurrence note, "all set"), each filled in all six `const` tables of `i18n.rs` (a missing one
   is a compile error). A per-feature test in the `layer_strings_091` pattern asserts non-empty, same `{}`
   count as English and not equal to English.

### WS-D — Cells that take a click (R13–R18), in the shared renderer

`render.rs` DataGrid arm (10026–12525), the only place that paints a grid on every surface. Every change to it
runs the DataGrid quality pass (R24) before the milestone is called done.

**D0 — One road for a changed cell (all milestones).** The push that follows a kept text edit (`render.rs`
≈12158–12170: `datagrid_commit_edit` → `prop_updates`, then `onCellEdited` with the 0-based
`"row,displayCol"`) is lifted into `push_cell_edit()` and called by the text editor, the Checkbox and the
Dropdown. 094's `note_grid_edit` / `settle_grid_edits` therefore apply to a toggle or a pick unchanged (spec §2
last finding).

**D1 — Checkbox (M3).**
- `DataGridColumn` gains `checked_value` and `unchecked_value` (`#[serde(default)]`, empty = derive; the column
  editor shows them as *Checked value* / *Unchecked value*, the documentation names them `CheckedValue` /
  `UncheckedValue`). Derived pair: from the cell's own text among `Y/N`, `y/n`, `1/0`, `true/false`,
  `True/False`, `TRUE/FALSE`, `yes/no`, `X/space`; `Y/N` for an empty or unrecognised cell.
- One pure pair in `datagrid.rs` — `checkbox_checked(raw, col)` and `checkbox_next(raw, col)` — used by **both**
  the paint (replacing the inline `y|yes|true|1|x|checked` set at ≈11944) and the toggle, so "the painted box and
  the written value always agree" (R14). An explicit pair makes the cell checked iff `raw == checked_value`; with
  none, the old truthy set keeps painting what it painted.
- Click on a Checkbox cell, or Space on the selected one (the grid consumes Space only here), when
  `allow_cell_edit` (which already includes `enabled`) and the column has data behind it: build a `DataGridEdit`
  whose text is the next value and call `push_cell_edit`. `column_editable` is split into *text-editable* (no
  longer true for Checkbox/Dropdown) and *click-editable*, so a double-click on one of these cells is two toggles
  and opens no text box.
- The cell's `ui.interact` is registered when `SelectableText` **or** the cell is click-editable, and its hit
  rect is `cell_rect ∩ visible body` (the precedent is `edit_rect`, ≈11606), so a row cut by the bottom edge or a
  column under the frozen band cannot steal a click. Button, Gauge, Image and plain cells are untouched (R18),
  and `onCellClick` still fires for the cell as today. A pointing-hand cursor marks an actionable cell.

**D2 — Dropdown (M4).**
- *Persisted config (F1).* A `DataGridDropdown { origin, source, value_field, display_field, limit, items }`
  (origin: `Static`, `CobolTable`, `IndexedFile`, plus the editor's two other origins stored but not resolved)
  is added to `BindingField` and `DataGridColumn`, both `#[serde(default)]`. The binding editor's modal already
  edits a `DropdownConfig`; it now saves and restores it, and `merge_datagrid_binding_column` copies it onto the
  column and keeps it on re-apply. The column editor (≈5604) gets the *Static items* text (`CODE=Label` per
  line) for a grid that is not bound.
- *Items (the renderer cannot read COBOL memory).* The interpreter resolves a column's items after the load and
  on `RefreshBinding()` — from a COBOL table's two OCCURS items, or an indexed file's two fields through
  `load_indexed` + `format_field_display`, capped at `limit` — and publishes them as the runtime-only
  `_ColumnItems`. Static items are read from the column and need no interpreter.
- *The cell.* Q3 default: the cell holds the **value** and paints the **display text** of the item whose value
  matches (the raw text when none does). Click, or F2/Alt+Down on the selected cell, opens the list; Up/Down,
  Enter and Escape are the popup's own keys.
- *The popup.* The ComboBox's popup pass (`render_form_inner` ≈3311) is reused rather than re-implemented:
  `OpenCombo` gains a `target` (`Control` as today, or `GridCell { row_index, display_col, rows, display_cols }`),
  and a `Select` for a grid cell goes through `push_cell_edit` instead of writing `Value`/`SelectedIndex`. The
  popup's z-order, theme fills, scrolling, outside-click close and the Spatial see-through viewport come with it.
  Open/closed state is `ctrl_id.with("datagrid-dropdown")` in egui memory, like the grid's other temp state.

**Designer.** Edit mode already forces `AllowCellEditing` on its render copy and lets `Rows` through, so a
toggle or pick works — and undoes — on the canvas with no further change; `EditedValue`/`PreviousValue` are not
saved to the form.

### WS-E — Closing (R25, R26, AC10–AC12)

- **System KB** (`cobolt-compiler/src/lib.rs`): `property_reference` — `SavedRows` ("integer (runtime-only,
  read-only)"; the domain must contain "runtime-only" for `every_runtime_only_property_is_listed_for_some_control`),
  `CommitStatus` with `94`/`95`, `AutoCommit` and `AllowCellEditing` completed; `event_reference` — `onSaved`
  beside `onCommitError`, whose text ("the cell already shows its old value again") is made true for both
  presentations; `control_method_docs("DataGrid")` — `Save() → String`, `CommitEdit` text; the compact table —
  `Save`, `CommitEdit`, `DiscardEdit` (F9); the Columns/AdvancedGrid text — `CheckedValue`, `UncheckedValue`,
  Dropdown items. IDE completion, the designer's method validator, Grace and the plugin pack fold the method
  and event lists in automatically (`editor.rs` ≈656, `reference.rs:361` checks it).
- **`assets/knowledge/chunked.data`** regenerated in every commit that edits those tables (M1, M3, M4):
  `cargo run -p cobolt-ide --example build_chunked_kb`, needing the multilingual-e5-small model
  (`semantic_model_is_ready()`) — checked first at the start of M1; if it is not installed the operator is asked
  before work goes on. Verified by `prebuilt_chunked_kb_matches_the_published_documentation`.
- **Developer's Guide** (English only; `developers-guide-es/pt/jp/cn` are already gone from `docs/`, nothing to
  delete): *Saving an edit back to the source* (≈3717–3797) becomes *… `Save()`, `AutoCommit`, `DiscardEdit()`*
  — the three gates (bound + writable + `AllowCellEditing`), the transaction per source, the `CommitStatus` table
  with `94`/`95`, the two failure presentations, an example for a COBOL table and one for SQL, the identity
  rule; the *Data binding and the Guardian* lines about recoverable edits (≈3619–3624) and the keyless-table
  exemption; *Advanced DataGrid* for the two cell kinds and their column properties; the DataGrid events
  sentence (≈7093) and methods row (≈7382). `docs/cobol-support-matrix-en.md` has no DataGrid rows and gets none.
- **Report** `save_report` (Golden Rule 7), **versions** `crates/cobolt-ide/src/version.rs` and `CHANGELOG.md`.

### Milestones

| M | Commit | Content | KB regen |
|---|--------|---------|----------|
| M1 | 1.91.21 | WS-A + WS-B + their KB/Guide text + the `save_report` | yes |
| M2 | 1.91.22 | WS-C (Designer switch, Guardian, strings) + its Guide lines | no |
| M3 | 1.91.23 | WS-D0/D1 Checkbox + quality pass #1 + KB/Guide | yes |
| M4 | 1.91.24 | WS-D2 Dropdown + quality pass #2 + KB/Guide | yes |

M4 can wait for a later session without leaving M1–M3 incomplete (spec §10, last risk).

### Requirement coverage

R1–R2 → C1–C2 · R3 → A5, C3 · R4 → C4 · R5 → B · R6 → A2–A5 · R7, R8 → A6 · R9 → A2, A6 · R10 → A2 ·
R11 → A2, B · R12 → A2 · R13, R14, R16–R18 → D0, D1 · R15, R16 → D2 · R19–R21 → A5, A6 · R22 → §6 AC8 ·
R23 → §6 AC9 · R24 → §6 AC10 · R25, R26 → E.

## 2. Affected crates / files

- `cobolt-runtime/src/interpreter/grid_edit.rs` — `PendingEdit.id`, `RowId`, `row_identity`, `locate`, `note`,
  settle, `save_method`/`commit_edit_method`/`discard_edit_method` over `commit_unit`, outcome and markers.
- `cobolt-runtime/src/interpreter/grid_unit.rs` — **new**: collect/group, the three planners and appliers, the
  status constants (`94`, `95` added).
- `cobolt-runtime/src/interpreter.rs` — `"SAVE"` arm, `_RowOccurrences` in `refresh_datagrid_binding` and the
  `Sort`/`DeleteRow`/`AddRow`/`ClearRows` arms, `reapply_pending_edits`, `refresh_dropdown_items` (M4),
  `mod grid_unit;`.
- `cobolt-runtime/src/db_runtime.rs` — `DbConn::in_transaction`, statement-keyword scanner, `DbRegistry::
  in_transaction`, `record_statements`. `indexed_ide.rs` — `GridSession::position_of_key`.
- `cobolt-ast/src/methods.rs` — `"SAVE"`.
- `cobolt-forms/src/model.rs` — DataGrid event `onSaved`; runtime properties; `DataGridColumn.checked_value`,
  `unchecked_value`, `dropdown` (M4); `DataGridDropdown` (M4); `BindingField.dropdown` (M4).
- `cobolt-forms/src/datagrid.rs` — `checkbox_checked`, `checkbox_next`, dropdown item parsing/lookup.
- `cobolt-forms/src/render.rs` — `push_cell_edit`, Checkbox/Dropdown cell paint and input, `_FailedCells` mark,
  `OpenCombo` target, `column_editable` split.
- `cobolt-project-tools/src/binding_guardian.rs` — keyless COBOL table.
- `cobolt-ide/src/panels/properties.rs` — editor state/mode, `show_writable_section`, `writable_needs`, column
  editor fields, dropdown persistence. `cobolt-ide/src/app.rs` — `merge_datagrid_binding_column` keeps the new
  column fields. `cobolt-ide/src/i18n.rs` — new `Tr` fields ×6. `cobolt-ide/src/version.rs`.
- `cobolt-compiler/src/lib.rs` — property, event, method tables. `assets/knowledge/chunked.data` — regenerated.
- `docs/developers-guide-en.md`; `CHANGELOG.md`.
- Tests: `cobolt-runtime/tests/test_grid_write_back.rs` (extended), `cobolt-forms/tests/` (new
  `datagrid_cells_095.rs`), in-file tests in `render.rs`, `datagrid.rs`, `db_runtime.rs`, `binding_guardian.rs`,
  `properties.rs`, `designer_grid_edit_094_tests.rs`, `xml.rs`, `i18n.rs`.

## 3. Data / model changes

- `.cfrm`: no schema change. `BindingMode` is already saved (`"mode": "Writable"`). New optional column fields
  live inside the `AdvancedGrid` JSON and are `#[serde(default)]`, so older forms load unchanged and a form saved
  by this version still loads in the previous one (unknown fields are ignored). `BindingField.dropdown` is
  likewise defaulted.
- Runtime-only properties (never saved): `SavedRows`, `_FailedCells`, `_RowOccurrences`, `_ColumnItems`.
- Status codes: `94` a SQL transaction is already open, `95` the connection is in `Async` mode. Existing codes
  keep their meaning. `CommitEdit()` with nothing pending answers `00` (I3).

## 4. Key decisions & alternatives

- **D1 — Plan, then apply, in one transaction per source.** Why: validation errors (PICTURE, key, vanished row)
  are the common failure and cost nothing to catch before any write. *Rejected:* a loop of per-cell writers with
  compensating undo — it is 094's shape and is what made a partial commit possible.
- **D2 — Group by source row, one `UPDATE`/`REWRITE` per row carrying all its columns** (R6). *Rejected:* one
  statement per cell — more statements, and a row could be half-written inside the unit.
- **D3 — Pending edits carry the row's identity from the moment they are noted** (F3). Why: a batch waits
  across the operator's and the program's actions. *Rejected:* trusting the position — correct only until the
  first `Sort`.
- **D4 — Transaction state: driver for SQLite, a statement-keyword flag for PostgreSQL/MySQL.** Why: it is the
  only door for these two drivers, and the EXEC SQL backends already do the same. *Rejected:* probing with a
  query (it would clobber the connection's result set; the portable PostgreSQL probe needs a query too) and a
  second connection for the unit (it cannot see the program's own uncommitted work, and `:memory:` SQLite is per
  connection). Known limit, documented: a transaction begun any way other than SQL text is not seen — notably
  MySQL's `BEGIN` implicitly commits an open one, which is why the guard exists.
- **D5 — The grid's `AutoCommit` decides what a failed unit leaves behind.** Off: nothing is lost — values and
  pending stay, the failing cell is marked (R7/Q6). On: 094's behaviour — cells go back (`a_value_too_long…`,
  `an_edit_to_the_key_column_is_refused` and the other 094 tests assert it, AC8). *Rejected:* "always keep the
  values": breaks AC8 and would make the settle retry a failing unit at every `COBOL-WAIT-EVENT`.
- **D6 — Occurrence identity by a parallel `_RowOccurrences`, not a hidden `Rows` column** (Q2; the spec's
  default was the hidden column). Why: every consumer of `Rows` — `GetCellValue`, `ExportCSV`, copy, filters,
  `AddRow(text)`, the renderer's cell counts — would have to learn to skip it; the per-row-height list already
  proves the parallel-property pattern against `Sort`/`DeleteRow`. *Rejected:* the hidden column; "require a key
  after all" (R3 asks for the opposite).
- **D7 — One function behind `Save()` and `CommitEdit()`** (Q1, R11 as written). `Save()` is the documented
  name.
- **D8 — Checkbox and Dropdown enter through `datagrid_commit_edit`** via `push_cell_edit`. *Rejected:* a second
  event path — it would skip 094's note/settle and its refusal rule.
- **D9 — Dropdown items are published by the interpreter, the popup is the ComboBox's.** Why: `cobolt-forms`
  has no access to COBOL memory or files at paint time, and a second popup would drift from the ComboBox's
  theme and keyboard rules. *Rejected:* `egui::ComboBox` (not the product's look) and resolving items in the
  renderer.
- **D10 — The editor computes "what the grid still needs" itself** (`writable_needs`). *Rejected:* a Guardian
  probe form per frame — the Guardian is a Save/Run gate with whole-form scope, not an editor widget.

### Interpretations I made — for the operator's eye

- **I1** `AutoCommit` on keeps 094's revert-on-failure (D5), a narrower reading of R7 than "always pending".
- **I2** `91`, `94`, `95` are refusals: status and `LastError`, **no** `onCommitError` (no row failed).
- **I3** `CommitEdit()` with nothing pending answers `00` like `Save()` (R9, R11); 094 returned the held status.
- **I4** `onSaved` fires whenever a unit wrote at least one row — from `Save()`, `CommitEdit()` and an
  `AutoCommit` settle alike; an empty `Save()` raises nothing.
- **I5** Q2 resolved with `_RowOccurrences`, against the spec's stated default (D6).
- **I6** Q3, Q4, Q5, Q6 follow the spec's defaults; Q5 is code `95`.
- **I7** AC12 is met by a Rust `save_report` block as in 094, not a `tests/cobol` program (F6).
- **I8** Dropdown (M4) is its own commit and may ship later; its origins resolve at run time only for
  *Static*, *COBOL table* and *Indexed file* (F1).
- **I9** A double-click on a Checkbox cell is two toggles and opens no text box.

## 5. Risks & mitigations

- **A transaction around a connection the program also uses** (spec §10). *Mitigation:* the guard `94`; the
  statement-flag limit is documented, tested with `BEGIN`, `START TRANSACTION`, `SET autocommit=0`, a
  `ROLLBACK TO`, and a keyword inside a quoted string. PostgreSQL/MySQL paths cannot be run here (no server):
  they share the SQLite-proven code path and the scanner's unit tests; the report says which engines ran.
- **MySQL reports 0 affected rows for an `UPDATE` that changes nothing**, which the unit reads as "no such row"
  (`23`). 094's single-cell write has the same limit; unchanged, noted in the Guide.
- **Rendering regressions in the busiest control.** *Mitigation:* R24 is a gate at M3 and M4; Textbox/Button
  painting is not touched, so the PowerChat/PowerDemo3 corpus goldens must stay byte-identical (no example has a
  Checkbox/Dropdown column) — any diff is a regression.
- **A click that changes data.** Gated by `AllowCellEditing` and `enabled`; the Designer switch shows the gate
  (R2); the hit rect is clipped to the visible body; tested with a clipped row and a frozen-column overlap.
- **Dropdown scope.** F1 is real new work (model, editor, resolver, popup). Mitigation: M4, separable.
- **`is_known_method("SAVE")`** makes `Save` unreadable as a property on every control. No control has one; the
  scan test and the DataGrid-only arm limit the blast radius.
- **`chunked.data` needs the embedding model.** Checked at the start of M1.
- **Save cost on a large indexed file** (one scan per rewrite). Measured in the report before any optimisation.
- **Two parallel sessions on this tree.** The checkout is shared; `main` also carries an uncommitted golden
  refresh from another session (not this feature's).

## 6. Test strategy

Every new test prints its cases; the closing `save_report` prints the Golden Rule 7 block (cases by name, ms,
`TESTS RUN` / `TESTS FAILED` / `OVERALL RESULT`) — verify-first, nothing asserted that the run did not produce.

- **AC2 / AC3 / AC4 / AC5 / AC6** — `cobolt-runtime/tests/test_grid_write_back.rs`: `LOOP` gains an `onSaved`
  branch; `Run` gains the recorded SQL statements. New tests: indexed atomic save (3 cells in 2 records → both
  rewritten, one commit, `SavedRows` 2, `onSaved` once); indexed rollback (bad PICTURE in the second record →
  neither changed, both pending, `onCommitError`); SQL (`BEGIN, UPDATE, UPDATE, COMMIT`; a failing second row
  leaves the first unchanged; a program `BEGIN` → `94`, nothing written, the program's transaction intact;
  `Mode=Async` → `95`); table (autocommit stores; `AutoCommit` off stores nothing until `Save()`; one
  non-numeric value stores none; keyless table via occurrence, including after `Sort` and `DeleteRow`); empty
  `Save()` (no `BEGIN`, `00`); `CommitEdit()` ≡ `Save()` on the same pending set; a handler making two edits is
  one unit; a `Sort` between edit and `Save()` writes the right record; a pending set survives `RefreshBinding()`.
  The 21 existing tests unchanged (AC8).
- **`db_runtime.rs`** unit tests: the statement scanner and `in_transaction` on SQLite.
- **AC1** — `properties.rs` tests (template: `from_existing_reopens_saved_binding_config_for_editing`): new
  binding read-only; Writable saved and reopened; `writable_needs`; keyless COBOL table validates;
  `xml.rs` round-trip of `"Writable"`; Guardian test beside `…blocks_missing_identity_for_writable`; a `Rig` test
  in `designer_grid_edit_094_tests.rs` for the one-click fix and its undo; the six-language string test.
- **AC7** — `render.rs` in-file, `--features render`, using `drive`/`audit_grid` (template:
  `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed`): click toggles and raises `onCellEdited` with the
  four values; Space; disabled grid and `AllowCellEditing` off do nothing; explicit `CheckedValue`/`UncheckedValue`
  written and painted alike; `datagrid.rs` unit tests of `checkbox_checked`/`checkbox_next` over every pair;
  Dropdown opens, picks, Escape and outside-click change nothing; value stored, text painted; Button, Gauge,
  Image unchanged; clipped-row and frozen-overlap clicks.
- **AC9** — `cobolt-forms/tests/datagrid_cells_095.rs`: one grid with a Checkbox and a Dropdown column through
  `render_form`, `render_form_with_chrome` and `render_form_scoped`, equal digest of the shapes in the grid rect;
  the 056 corpus goldens unchanged. Interpreter parity: the Save scenarios through the interpreter as built by
  each of the four sites (`rcrun run-form`, child-form host, `run_form_app`, `run-form-ipc`), plus the
  `interpreter-binary-parity` checks: `cargo test -p cobolt-runtime -p cobolt-form-host -p cobolt-compiler
  -p cobolt-cli`; the commit message names the hosts touched.
- **AC10** — the DataGrid quality pass (`.agents/skills/datagrid-quality`): a short one at M1 on the failed-cell mark
  (the only `render.rs` change there), full ones at M3 and M4, reports saved beside the
  spec; its 18 scenarios, especially 11–13 (embedded cells), 16–18 (virtual scrolling, frozen panes) and the
  layer order. Scenarios 12 and 13 have no test today; they get one each.
- **AC11** — `prebuilt_chunked_kb_matches_the_published_documentation`,
  `every_runtime_only_property_is_listed_for_some_control`, `pack_names_every_kb_entry_and_builtin`,
  `every_dispatched_control_method_is_spellable_inline`, `intellisense_offers_every_method_the_knowledge_base_documents`;
  `Save` accepted by Check and the validator through a handler fixture.
- **Manual** (never drive the IDE window): a scratch form with Checkbox and Dropdown columns through `run_form`
  and the Run Form process; the Designer's canvas look is the operator's to judge.

## 7. Steering compliance

- [ ] i18n: every new editor string a `Tr` field in all six languages (WS-C4; M3/M4 column-editor strings too)
- [ ] Generated-code banner + regenerate-on-action contract preserved (codegen unchanged; no new seed)
- [ ] English Developer's Guide updated; translations not touched (already absent from `docs/`)
- [ ] System KB tables + `chunked.data` regenerated in the same change (M1, M3, M4)
- [ ] Four interpreter hosts (F5); `interpreter-binary-parity` checklist run
- [ ] DataGrid quality pass at M1 (failed-cell mark, short), M3 and M4 (R24)
- [ ] Fix vs feature: **feature** → `features-datagrid-save`, `z` bump per milestone (1.91.21 → 1.91.24) and a
  CHANGELOG entry each; never `x` or `y`; no push (work-hours rule), no merge into `main` until asked
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
