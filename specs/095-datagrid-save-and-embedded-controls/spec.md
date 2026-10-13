# Spec — DataGrid: reach write-back from the Designer, `Save()` in one transaction, and cells that take a click

- **Status:** approved by the operator on 2026-10-12 with the decisions of §8; amended 2026-10-13 (§8.1: Save() writes inserts and deletes too, the check is in, a failed save restores); implementing — see tasks.md
- **Folder:** specs/095-datagrid-save-and-embedded-controls/
- **Author:** operator request, written by Claude   **Date:** 2026-10-12
- **Builds on:** spec 094 (*DataGrid write-back*, shipped as 1.91.15). 094 is read, not
  re-opened: every decision of its §8 stands, and this spec says where it adds to one.

## 1. Overview
The operator asked, in one sentence: when data binding is active, an edit made in the
grid — *using the controls embedded in the grid* — must reach the COBOL table the grid is
bound to; and for a database association, a method `::Save()` shall start a transaction,
update **every record loaded in the grid that changed**, and commit.

Most of that is already built (spec 094) and the request was written without it in
sight, so this spec starts from what 094 shipped and states only the gap. Four things
stand between the request and a working feature, each read in the source on 2026-10-12,
none recalled:

1. **A COBOL-table or SQL binding cannot be made writable from the Designer.** The
   run-time gate needs `_BindingWritable = 1`, which comes from a binding's
   `BindingMode::Writable`. The Data Binding editor has no mode and no writable switch
   — `BindingEditorState` carries neither, and `to_binding` always builds the binding
   with `DataBindingDef::new`, which is read-only. Only a quick-bound indexed grid, which
   has no binding definition, is writable from the Properties pane. For a COBOL table
   this is the whole reason an edit "does nothing": the write-back exists
   (`write_table`) and is switched off with no way to switch it on.
2. **There is no `Save()`, and a commit is not atomic.** 094's `CommitEdit()` writes the
   pending edits **one by one** (`commit_grid_edits` → `commit_one`). `write_sql` issues
   one `UPDATE` per edited *cell* and says so in its own comment — it "commits … at once";
   `write_indexed` opens a session, rewrites and commits **per edit**. A failure
   restores one cell and the loop carries on: the edits before it are already committed.
   That is a partial commit, not the transaction that was asked for.
3. **A Checkbox or Dropdown column is painted, not alive.** `render.rs` draws a box for a
   Checkbox column and a "▼" glyph for a Dropdown column; neither takes a click, and no
   event comes from either. The only embedded editor a grid has is the text box that
   `AllowCellEditing` opens. So "a change made with an embedded control" can only be a
   text edit today.
4. **Edits to a bound table are three gates deep.** 094's R8 requires a bound grid, a
   writable binding *and* `AllowCellEditing` on. With (1) fixed, a developer who binds a
   table and ticks "writable" still sees nothing happen until they find a third switch.
   The Designer shall say what is missing.

## 2. Findings — what 094 left, with the code that shows it
- **Pending is a list, which is what a batch needs.** `GridEditState { pending:
  Vec<PendingEdit> }` (`interpreter/grid_edit.rs`) keeps every edit the code has not yet
  decided. `PendingEdit` is `{ row, col, value, previous }` — one *cell*, not one row.
- **Three writers, one cell each.** `write_indexed`, `write_table`, `write_sql`, each
  locating the source row **by its key cells as the grid shows them** (never by position)
  and refusing an edit to a key column (`21`). That identity rule is right and stays.
- **No unit of work.** A `COMMIT`/`ROLLBACK` exists for the indexed engine
  (`GridSession::commit`/`rollback`) and, for SQL, as plain `BEGIN`/`COMMIT`/`ROLLBACK`
  text sent through the `SqlDatabase` control's handle (`db.exec`), which owns one
  persistent connection; a table has none. Nothing groups a grid's edits into one.
- **SQL values are literals, escaped by the runtime.** The control's `exec` takes text
  only; `sql_literal` quotes and checks each value and `is_plain_name` guards every
  identifier. That stays the way a value reaches a statement (no parameter API exists on
  that path).
- **A Dropdown column already has configuration, and no behaviour.** The binding editor
  has a per-field Dropdown configuration (origin: a COBOL table or an indexed file —
  `show_dropdown_config_modal`), saved in the binding; the grid draws "▼" and ignores it.
- **The grid's edit pipeline is single-sourced.** A kept edit becomes `Rows`,
  `EditedRow`, `EditedColumn`, `EditedValue`, `PreviousValue` and `onCellEdited`
  (`datagrid_commit_edit`, `render.rs`); 094's `note_grid_edit` then keeps it pending and
  `settle_grid_edits` writes it after the handler returns. Anything new that changes a
  cell shall enter through this one road, so 094's handler-can-refuse rule and its
  writers apply to it unchanged.

## 3. Goals / Non-goals
- **Goals**
  - A developer can make a data binding **writable from the Designer**, sees what a
    grid still needs to write back, and the form works the first time (§4.1).
  - `grid::Save()` writes **all the changes made since the last load or save** in
    **one transaction**: all or nothing (§4.2).
  - A **Checkbox** cell toggles on a click and a **Dropdown** cell opens a list and
    picks a value; both commit through the same road as a text edit, so they write back
    exactly like it (§4.3).
  - A COBOL table is updated by any such change when the binding is writable, without
    code (§4.4).
- **Non-goals**
  - ~~Adding or deleting rows of a bound grid~~ — **now in scope** (§8.1, D3).
  - Date picker, numeric up/down, progress or user controls inside a cell, and an
    embedded ComboBox *control* — the two cell kinds above are the scope.
  - Write-back for REST or Agent sources (spec 022).
  - ~~Optimistic concurrency~~ — **now in scope** (§8.1, Q4).
  - A new binding model, storage engine or event loop.

## 4. Requirements (EARS)

### 4.1 Writable from the Designer
- **R1 (ubiquitous):** The Data Binding editor shall offer a **Writable** switch for a
  binding whose source can be written (Indexed, SQL, COBOL table), **off by default**,
  and shall save it as the binding's `mode`. A binding that already exists keeps the
  mode it was saved with; no saved form changes behaviour.
- **R2 (state):** While Writable is on, the editor shall show what the grid still
  needs, in plain words, beside the switch: the **key fields** (Indexed and SQL: marked
  with the existing *Key* checkbox) and **Allow cell editing** on the grid. A missing
  item shall be a one-click fix where one exists (turn on cell editing) and a pointer to
  the field where it does not.
- **R3 (constraint):** For a **COBOL table**, the row identity the Guardian asks of a
  writable binding shall be satisfied by the table's own key fields *or*, when the
  developer marks none, the occurrence — so that a table with no natural key can be
  written back. (094's `write_table` finds the row by key cells; the plan decides how an
  occurrence-identified table is found, see Q2.)
- **R4 (constraint):** Every new editor string shall be a `Tr` field in all six
  languages.

### 4.2 `Save()` — one transaction
> **Amended by §8.1 (2026-10-13).** Where R6–R12 and R19–R21 differ from §8.1, §8.1 wins: `Save()` also inserts and
> deletes rows (blank lines never), checks the source against what the grid loaded (`96`), puts the old values back
> when it fails, joins a transaction the program began (status `94` is retired), and regenerates a COBOL table
> whole from the grid; `CommitEdit()` is a deprecated alias.

- **R5 (ubiquitous):** The DataGrid shall have a **`Save()`** method. It acts on a grid
  that is writable (094's R8) and answers a status the code can test, the same one
  `CommitStatus` holds.
- **R6 (event):** When `Save()` is called, the runtime shall write **every pending
  edit** of the grid inside **one transaction** on the grid's source, **grouped by row**
  — one `UPDATE`/`REWRITE` per changed row carrying all of that row's changed columns —
  and then commit.
  - **Indexed:** one session; every changed record rewritten; one engine `COMMIT`; on
    any failure the engine's `ROLLBACK`.
  - **SQL:** `BEGIN`, the `UPDATE`s, `COMMIT`; on any failure `ROLLBACK`. If the program
    has begun a transaction of its own on that connection, `Save()` shall not end it:
    it reports a status of its own ("a transaction is already open") and writes nothing,
    so the program's `COMMIT` stays the program's. The plan picks the code: it must not
    reuse one of 094's (`00`, `21`, `23`, `90`, `91`, `92`, `98`) nor a FILE STATUS value
    (`93` is the indexed engine's "resource not available").
  - **COBOL table:** all values are validated first (PICTURE, numeric), then assigned;
    one invalid value assigns none.
- **R7 (event):** When any row of a `Save()` fails, the runtime shall **roll back the
  whole unit**, shall leave every edit **pending and recoverable** (094's R13 / Guide
  §2.2: the pending edit and row identity remain), shall set `CommitStatus` and
  `LastError` to the failing row's code and text, and shall raise **`onCommitError`**
  with the failing `EditedRow`/`EditedColumn`. No row is committed.
- **R8 (event):** When `Save()` succeeds, the runtime shall clear the pending edits,
  show the values the source holds (094: `5` in a `9(4)` field is `0005`), set
  `CommitStatus` to `00`, and raise **`onSaved`** once, with the count of rows written
  in **`SavedRows`**.
- **R9 (constraint):** `Save()` with nothing pending shall not open a transaction and
  shall answer `00` with `SavedRows` 0.
- **R10 (constraint):** The same unit of work shall be what `AutoCommit` on does at the
  end of each handler: **all the edits kept by that event are written as one unit**, not
  cell by cell. (094 writes each pending edit separately there too.)
- **R11 (ubiquitous):** `CommitEdit()` keeps its name and its meaning to a form written
  against 1.91.15 — *write what is pending* — and shall do so through the **same unit of
  work** as `Save()`. The two answer the same; `Save()` is the name the documentation
  teaches (Q1).
- **R12 (constraint):** A key column is never written (094: status `21`). A failure of
  one of several edits to a key column fails the unit like any other.

### 4.3 Cells that take a click
- **R13 (event):** When the operator clicks a **Checkbox** cell, or presses Space on the
  selected one, of a grid with `AllowCellEditing` on, the grid shall toggle the cell,
  write the new value into `Rows` and raise `onCellEdited` with the same
  `EditedRow`/`EditedColumn`/`EditedValue`/`PreviousValue` a text edit raises.
- **R14 (ubiquitous):** What a Checkbox cell holds when checked and when unchecked is
  the column's: two column properties, `CheckedValue` and `UncheckedValue`, that default
  to the pair the cell's data already uses (`Y`/`N`, `1`/`0`, `true`/`false`, `X`/space)
  and to `Y`/`N` when the cell is empty. The painted box and the written value shall
  always agree.
- **R15 (event):** When the operator clicks a **Dropdown** cell, or presses F2/Alt+Down
  on the selected one, of a grid with `AllowCellEditing` on, the grid shall open a list
  anchored to the cell, drawn like the ComboBox control's, with the items the column's
  configuration gives (§2: a COBOL table or an indexed file). Picking an item writes it
  into `Rows` and raises `onCellEdited` as above; Escape or a click outside closes the
  list and changes nothing.
- **R16 (ubiquitous):** Both kinds shall be operable from the keyboard (Space, Up/Down,
  Enter, Escape), keep the grid's selection, hover and focus states, and honour a
  disabled or read-only grid by doing nothing.
- **R17 (constraint):** Nothing about a Checkbox or Dropdown cell changes when
  `AllowCellEditing` is off: it is painted exactly as today (094's R20).
- **R18 (constraint):** The Button, Gauge and Image cell kinds are unchanged: they stay
  non-editable, and `onCellClick` still fires as it does.

### 4.4 A COBOL table, without code
- **R19 (event):** When a writable grid bound to a COBOL table keeps an edit — a text
  edit, a Checkbox toggle or a Dropdown pick — and `AutoCommit` is on, the runtime shall,
  after the `onCellEdited` handler returns, store the cell's current value in the table
  item at the row's occurrence, as a `MOVE` would (the PICTURE applies; a number must be
  one). A handler that restored `PreviousValue` has refused the edit and nothing is
  stored (094's R9).
- **R20 (event):** When `AutoCommit` is off, the edits wait for `Save()`/`CommitEdit()`
  or `DiscardEdit()`, as 094's R10–R11.
- **R21 (constraint):** A value the table item cannot hold (non-numeric text in a
  numeric item) is refused, the cell shows the table's value again, and the status is
  `92`, exactly as 094 does for one cell.

### 4.5 Compatibility, parity, documentation
- **R22 (constraint):** No existing form changes behaviour: a grid that is not bound, is
  read-only, or has editing off is untouched; a binding saved read-only stays read-only;
  a handler that stores the edit itself in `onCellEdited` keeps working.
- **R23 (constraint):** The run-time change shall reach all three hosts that construct
  the interpreter (`rcrun run-form`, embedded child forms, the compiled binary's
  `run_form_app`); it lives in the interpreter and the shared renderer, not in a host
  (`interpreter-binary-parity`). The renderer change shall draw the same cell on the
  Designer canvas, Preview, Run Form and the compiled binary (spec 017's one engine).
- **R24 (ubiquitous):** The DataGrid quality pass (`.agents/agents/datagrid-quality-
  compatibility-agent.md`) shall be run on every change to the grid's rendering,
  interaction or binding, and its 18 regression scenarios kept green — in particular
  scenarios 11–13 (embedded TextBox, ComboBox and CheckBox cells), 16–18 (virtual
  scrolling and frozen panes) and the layer order of §"Validation Checklist" (cell
  content, custom control, then selection, hover and focus, then grid lines).
- **R25 (ubiquitous):** The System KB tables shall carry `Save`, `onSaved`, `SavedRows`,
  `CheckedValue`, `UncheckedValue`, the Dropdown items and the completed text for
  `AutoCommit`, `CommitEdit` and `CommitStatus`, with `chunked.data` regenerated in the
  same change; `Save` shall be registered everywhere a method name must be (the closed
  inline vocabulary, the dispatch, the editor's completion and validation).
- **R26 (ubiquitous):** The Developer's Guide (*DataGrid*, *Data binding*) shall state
  the three gates, the transaction, the status codes and an example in COBOL for a
  COBOL table and for a SQL source; spec 094's §2.2 note and the Guardian paragraph
  shall be brought in line. Translations are deleted (Golden Rule 8).

## 5. Acceptance criteria
> AC2–AC4 and AC6 keep their scenarios; read their "edits stay pending" and "transaction-open status" as §8.1 now says
> (old values back; the save joins the program's transaction). §8.1 adds: **AC13** insert, update and delete in one
> unit over an indexed file and a SQL table (blank line never inserted, changed key never written, key read-only on
> a loaded row); **AC14** the changed-since-load check (`96`) for an update and a delete; **AC15** a COBOL table
> regenerated from the grid after an edit, a sort, an add and a delete; **AC16** `BeginTransaction()`/`Commit()`/
> `Rollback()`/`IsInTransaction()` and a save that joins and survives the program's transaction.

- [x] **AC1 — R1–R4.** Designer tests: a new binding is read-only; ticking Writable
  saves `mode = Writable` and a reopened form still has it; the editor names the missing
  key and the missing *Allow cell editing*; a COBOL-table binding with no key marked
  validates when Writable; every string exists in six languages.
  *Verified by:* `cobolt-ide`: `a_new_binding_is_read_only`, `reopening_a_writable_binding_and_applying_keeps_it_writable`, `writable_needs_names_what_the_grid_still_lacks`, `the_writable_section_lists_the_needs_and_the_fix_turns_cell_editing_on` (a real click), the six-language strings test; `cobolt-project-tools`: `a_writable_cobol_table_without_a_key_is_allowed`.
- [x] **AC2 — R5–R9, indexed.** A bound indexed grid, `AutoCommit` off: edit three cells
  in two records, `Save()` → both records hold the new values after a re-read, one
  engine `COMMIT`, `SavedRows` 2, `onSaved` once. A bad PICTURE in the second record →
  **neither** record changed, `CommitStatus` set, both edits still pending,
  `onCommitError` raised.
  *Verified by:* `test_grid_write_back`: `three_cells_in_two_records_are_one_save`, `a_bad_value_in_the_second_record_changes_neither` (§8.1: the old values back, the cell marked, `onCommitError` once).
- [x] **AC3 — R6, R7, SQL.** Against the in-memory SQLite connection of the SQL tests:
  the same two scenarios; the statements the run sent are `BEGIN`, one `UPDATE` per row,
  `COMMIT` (or `ROLLBACK`); a constraint failure on the second row leaves the first row's
  value unchanged in the table. A program-opened transaction → the transaction-open
  status, nothing written, the program's transaction untouched.
  *Verified by:* `test_grid_write_back`: `a_save_sends_begin_one_update_per_row_and_commit`, `a_failing_second_row_leaves_the_first_unchanged`; §8.1: `a_save_joins_the_programs_transaction_and_leaves_the_commit_to_it`, `a_failed_save_inside_the_programs_transaction_undoes_only_its_own_writes`.
- [x] **AC4 — R6, R19–R21, COBOL table.** A writable grid bound to a WS table: a text
  edit with `AutoCommit` on stores the value; with `AutoCommit` off nothing is stored
  until `Save()`; a non-numeric value in a numeric item stores none of a multi-edit save.
  *Verified by:* `test_grid_write_back`: `with_auto_commit_off_nothing_is_stored_in_the_table_until_save`, `one_non_numeric_value_stores_none_of_a_multi_edit_save`, `a_table_binding_stores_the_edit_in_the_table_and_picture_rules_apply`.
- [x] **AC5 — R9.** `Save()` with nothing pending opens no transaction (the statement log
  has no `BEGIN`) and answers `00`.
  *Verified by:* `test_grid_write_back`: `an_empty_save_opens_no_transaction`.
- [x] **AC6 — R10, R11.** `AutoCommit` on, a handler that makes two edits (`SetCellValue`
  is not an edit; two cell edits in one event are) writes them in one unit; `CommitEdit()`
  and `Save()` give the same result on the same pending set.
  *Verified by:* `test_grid_write_back`: `auto_commit_writes_the_edits_kept_so_far_as_one_unit`, `commit_edit_and_save_give_the_same_result`.
- [x] **AC7 — R13–R18.** Render and interaction tests (`cobolt-forms`, headless): a click
  on a Checkbox cell toggles it and raises `onCellEdited` with the right values; Space
  does the same; a disabled grid and `AllowCellEditing` off do nothing; a Dropdown opens,
  picks, and Escape closes without a change; the column's `CheckedValue`/`UncheckedValue`
  are written and drawn consistently; the Button, Gauge and Image kinds are unchanged.
  *Verified by:* `cobolt-forms` render tests: the Checkbox set (`a_click_on_a_checkbox_cell_toggles_it_and_raises_on_cell_edited` …) and the Dropdown set (`picking_an_item_stores_its_value_and_raises_on_cell_edited`, `escape_and_a_click_outside_close_the_list_and_change_nothing`, `a_disabled_grid_or_editing_off_opens_no_list_and_paints_alike` …); `a_button_cell_is_still_only_a_click_when_editing_is_allowed`.
- [x] **AC8 — R22.** The existing 094 tests and `a_datagrid_cell_is_edited_in_place_when_
  editing_is_allowed` unchanged and green.
  *Verified by:* The 21 spec-094 tests and `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed` are green. Three of 094's expectations changed with the operator's answers (§8.1) — the old values back after a failure, the table regenerated, the record found by its load — each with the reason in the test.
- [x] **AC9 — R23.** The same scenarios through the interpreter that `rcrun run-form`, the
  child-form host and `run_form_app` each construct; a golden render of a Checkbox and a
  Dropdown cell equal on canvas, Preview and Run Form.
  *Verified by:* `cobolt-forms/tests/datagrid_cells_095.rs` (Checkbox 30 and Dropdown 34 identical shapes on run, preview and the Designer canvas); the interpreter is the one all hosts construct (no host edit).
- [x] **AC10 — R24.** The DataGrid quality agent's report attached, 18 scenarios green.
  *Verified by:* `quality-m3.md` and `quality-m4.md`.
- [x] **AC11 — R25, R26.** KB freshness test green with a regenerated `chunked.data`; the
  Guide updated; `Save` accepted by Check, completion and the validator.
  *Verified by:* The IDE suite's `prebuilt_chunked_kb_matches_the_published_documentation` passes on the regenerated `chunked.data`; `Save` is accepted by the vocabulary, the dispatch and the editor's completion test; the Guide is updated (the translations are deliberately not — the one allowed red, `docs_embed`).
- [x] **AC12 — Golden Rule 7.** A quantified program under `tests/cobol` reports the
  cases it ran (atomic save ok, atomic save rolled back, empty save, checkbox, dropdown,
  COBOL table on/off) and a `TESTS RUN` / `TESTS FAILED` / `OVERALL RESULT` tally.
  *Verified by:* The Rust test `save_report` (a console COBOL program has no event loop to carry a grid's report, so the block is printed from the Rust test, as 094's was): `TESTS FAILED 0000`.
- [x] **AC13 — D3.** `test_grid_write_back`: `a_save_deletes_updates_and_inserts_in_one_unit`, `nothing_inserts_or_deletes_until_save_is_called`, `a_blank_line_is_never_inserted`, `a_new_row_needs_its_key_and_a_duplicate_key_undoes_the_whole_unit`, `a_row_whose_key_was_changed_is_never_written`, `clear_rows_is_not_a_delete`, `a_sql_save_deletes_updates_and_inserts_in_one_transaction`, `a_sql_insert_needs_its_key_and_a_duplicate_undoes_the_whole_unit`, `a_sql_key_change_and_clear_rows_send_nothing`; `cobolt-forms`: `the_key_of_a_loaded_row_is_read_only_and_a_new_rows_is_not`.
- [x] **AC14 — Q4.** `a_row_changed_by_someone_else_is_not_overwritten`, `a_change_to_another_column_of_the_row_survives`, `a_row_changed_by_someone_else_is_not_deleted`, `a_sql_row_changed_by_someone_else_fails_the_save`.
- [x] **AC15 — Q2.** `a_table_is_regenerated_from_the_grid` (edit, Sort, DeleteRow, AddRow), `a_grid_with_more_rows_than_the_table_holds_is_refused`.
- [x] **AC16 — Risk.** `test_sql::the_transaction_verbs_commit_roll_back_and_refuse_what_cannot_apply` and the two joined-transaction tests of AC3.

## 6. Constraints & steering check
- **i18n:** yes — R4 (and any Designer string R2 adds).
- **Generated code:** the run-time route is preferred, as in 094; codegen may need to
  seed the column's `CheckedValue`/`UncheckedValue`/Dropdown items and the binding's
  mode (`_BindingWritable` already travels). Generated COBOL must stay parseable and pass
  `cobolt-semantic`.
- **System KB / Guide:** R25, R26.
- **Three hosts:** R23. **DataGrid guardrail:** R24 — the agent is called before the
  work is called done.
- **Fix vs feature:** **feature** (new method, new cell behaviour, new editor control).
  `features-datagrid-save`; a `z` bump and a changelog entry; never the minor.
- **Window rule:** the Dropdown list is a popup anchored to a cell and sized by the
  existing ComboBox rules, not a window that resizes itself.
- **Amendment to spec 022 R10b** ("source writes only through an explicit save/update or
  an existing form event contract"): for a **COBOL table** with `AutoCommit` on, 094
  already writes after the handler; this spec keeps that and adds `Save()` as the
  explicit batch. For **databases** the explicit action is `Save()` (or `CommitEdit()`),
  as R10b says, and `AutoCommit` remains the operator's choice of when.

## 7. Plan outline (what the plan should decide, in this order)
1. **The unit of work** (R6–R12), indexed first: one session, rows grouped, one commit;
   then SQL (`BEGIN`…, the transaction-open rule); then the table (validate-then-assign).
   Coalesce per row; keep 094's key-cell row finding.
2. **The Designer switch and its guidance** (R1–R3) — it makes everything else reachable.
3. **The cells** (R13–R18) in the shared renderer, with the DataGrid quality pass on each
   step; Dropdown items from the binding's configuration.
4. KB, Guide, six-language strings, the `tests/cobol` program.

## 8. Decisions (the operator's answers, 2026-10-12)
- **D1 — scope of `Save()`: SQL *and* Indexed**, one contract and two backends. A COBOL
  table gets the same method (R6) and, with `AutoCommit` on, no code at all (R19).
- **D2 — embedded controls: make them work, then write back.** A Checkbox cell toggles
  and a Dropdown cell picks; both write back like a text edit (§4.3). Not text only.
- **D3 — `Save()` writes changed rows only.** One transaction; not every loaded row; no
  `INSERT`/`DELETE` (094's D4 stands).

### 8.1 Amendment — the operator's answers of 2026-10-13 (they replace the defaults of §9)
D3 is widened and every question of §9 has its answer. Where this section and §4/§9 differ,
**this section wins**; the requirements below say what changes.

- **D3 (replaced) — `Save()` writes updates, inserts and deletes.** For an Indexed or SQL
  source, in one transaction: edited cells → `UPDATE`/`REWRITE` (one per changed row);
  rows added with `AddRow` → `INSERT`/`WRITE`; rows removed with `DeleteRow` → `DELETE`.
  **Not** blank lines (an added row with every cell blank is never inserted), and **never
  a row whose key has been modified** — an Indexed or SQL key is **read-only** on a row
  that came from the source (editable only on a row added in the grid, which has no key
  yet). Inserts and deletes are written only by an explicit `Save()`; `AutoCommit` still
  settles cell edits alone. `ClearRows()` empties the display and forgets what was
  loaded — it is not a request to delete the table. Replaces 094's D4 and the non-goal
  "adding or deleting rows".
- **Q1 — `Save()` is the name.** `CommitEdit()` stays as a deprecated alias for forms
  written against 1.91.15; the Guide stops mentioning it.
- **Q2 — a COBOL table is regenerated whole from the grid.** `Save()` (and `AutoCommit`)
  on a COBOL-table grid writes **every row of the grid** into the table's occurrences in
  grid order and blanks the occurrences after the last row: sorts, adds and deletes in the
  grid become the table. No key and no hidden occurrence column are needed. Blank rows are
  not written. Items of the table the grid does not show are left where they are.
- **Q3 — a Dropdown cell holds the value and shows the text.** When the item has display
  text, the cell shows the text and returns the value; when only a value is defined it
  shows and returns the value. (Built in M4.)
- **Q4 — the changed-since-load check is IN.** Before it writes, `Save()` compares what
  the source holds now with what the grid loaded: for an `UPDATE` the edited columns, for
  a `DELETE` every shown column of the row. A difference fails the unit with status
  **`96`** (the row was changed by someone else), writing nothing.
- **Q5 — default:** an `Async` connection refuses `Save()` with status `95`.
- **Q6 — a failed `Save()` puts the old values back.** The grid returns to what was last
  loaded or saved (edited cells, added rows and deleted rows undone), `CommitStatus` and
  `LastError` say why, `onCommitError` fires, and the cell that failed carries the red mark
  until the next edit. (Replaces "leave every edit pending".)
- **Risk — a transaction around a connection the program also uses.** The `SqlDatabase`
  control gets **`BeginTransaction()`, `Commit()`, `Rollback()` and `IsInTransaction()`**.
  `Save()` on a connection with a transaction open **joins it** under a savepoint — the
  program's `Commit()`/`Rollback()` stays the program's, and a failed save undoes only
  its own writes. Status `94` of the first plan is retired.
- **Risks — a click that changes data, rendering regressions, scope: accepted as
  planned** (the §4.3 gates, the R24 quality pass, three areas in one spec).

## 9. Open questions
- **Q1 — `Save()` and `CommitEdit()`.** Two names for one action is a cost: a developer
  has to learn both. R11 keeps `CommitEdit()` for forms written against 1.91.15 and makes
  `Save()` the documented name. Alternative: `Save()` only, `CommitEdit()` a deprecated
  alias that the Guide stops mentioning. *Default: R11 as written.*
- **Q2 — a COBOL table with no key.** R3 lets a table be writable by **occurrence**. 094
  finds the row by key cells in the edited row, because a position can move when the
  operator sorts or the code reorders. An occurrence-identified table has no key cells to
  find it by. The plan must choose: record the occurrence in a hidden grid column the
  refresh fills (survives a sort), or require a key after all. *Default: the hidden
  column.*
- **Q3 — a Dropdown's value.** Does the cell hold the item's *display text* or its
  *value* when the configuration has both? *Default: the value, shown as its text.*
- **Q4 — concurrent changes.** `Save()` finds each row by key and overwrites the columns
  that changed; another user's change to a *different* column survives, to the *same*
  column is overwritten. A "changed since load" check (compare the old value in the
  `WHERE`) is a follow-up unless the operator wants it here. *Default: not in this spec.*
- **Q5 — an `Async` `SqlDatabase`.** The control has a `Mode`. `Save()` on an async
  connection could run in the background and raise `onSaved` on completion, or refuse
  with a status. *Default: refuse with a status in this spec; async follows the control's
  own async verbs later.*
- **Q6 — what a rolled-back `Save()` shows.** Leave the edited cells showing the
  operator's values (still pending, red marker) so the operator can fix and retry, or put
  the old values back. 094 puts `PreviousValue` back on a failed *single* write. For a
  batch, putting 40 cells back loses 40 edits for one bad value. *Default: keep the
  values, mark the failing cell, keep everything pending.*

## 10. Risks
- **A transaction around a connection the program also uses.** The `SqlDatabase`
  control's handle has no transaction API of its own; `BEGIN`/`COMMIT` are text on a
  shared connection. R6's transaction-open rule is the guard; the plan must say how it *knows* a
  transaction is open (the driver, or a flag the control keeps).
- **A click that changes data.** A Checkbox that toggles is a bigger decision than one
  that is drawn. R13's gate (`AllowCellEditing` on) and R22 exist for that; the Designer
  shall make the connection visible (R2).
- **Rendering regressions** in the most heavily used control. R24 is a gate, not a note.
- **Scope.** Three feature areas share one spec; if the plan finds the Dropdown cell
  larger than the rest, §4.3 can ship as a second step behind the same pipeline without
  changing §4.1–§4.2.
