# Spec — DataGrid: edit visually in the Designer, and write edits back to the bound source

- **Status:** draft — findings and requirements; five questions open (§8) before `/plan`
- **Folder:** specs/094-datagrid-write-back/
- **Author:** operator request, written by Claude   **Date:** 2026-10-12
- **Number:** 093 is the COMPUTE shorthand, drafted on `features-assignment-statement` and not yet merged; this takes 094 so the two cannot collide.

## 1. Overview
The operator asked for three things:

1. a DataGrid that can be **edited visually in the Form Designer**;
2. a bound DataGrid whose in-cell edits at run time are **saved back to the
   source** — an indexed file, a COBOL table or a SQL connection — with an
   **auto-commit** option: on, the change is written; off, an **event** is
   raised so the code decides, **confirmed or discarded**;
3. **use whatever is already implemented.**

Point 3 matters most, because most of this is already built and not connected.
The binding model already says which sources are writable and what identifies a
row; the generated COBOL already keeps a dirty flag, a row key and a status per
binding; six `BINDING-…` built-ins already exist; the grid already edits a cell
in place and already raises `onCellEdited`; the indexed engine already has a
tested write path. What is missing is the wire between them — the built-in that
should write to the source writes nothing (§2) — and, in the Designer, a grid
that stops overwriting what the developer does to it (§2.6). This spec connects
what exists and adds the smallest new surface that the auto-commit choice needs.

## 2. Findings — what exists, and where it stops
Each line was read in the source on 2026-10-12, not recalled.

### 2.1 The binding model already knows what is writable
`cobolt-forms/src/model.rs` ≈1904–2070. A `DataBindingDef` has a `mode`
(`BindingMode::ReadOnly` | `Writable`, default read-only). Each source
descriptor carries `writable` and the fields that identify a row:
`IndexedFile { key_field, … }`, `Sql { source_control_id, query_name,
key_fields, … }`, `CobolTable { table_name, occurs_item, key_fields, … }`;
REST and Agent sources are writable only with explicit update metadata. The Data
Binding Guardian already blocks a writable binding with no key ("missing row
identity for writable bindings", Guide, *Data binding*).

### 2.2 The Guide already states the contract
*"Writable bindings must preserve source identity… Initial loads populate the
target without marking it dirty. User edits are kept as pending binding state
until an explicit update helper or your form's own event contract commits them;
if an update fails, the pending edit and row identity remain recoverable."*
That is requirement 2 of this spec. It was written; it was not built.

### 2.3 The built-ins are a state machine that writes nothing
`cobolt-runtime/src/builtins.rs` lists `BINDING-LOAD`, `-POPULATE`,
`-SET-PENDING`, `-UPDATE`, `-MARK-CLEAN`, `-SET-READ-ONLY`. In
`interpreter.rs` (≈11149–11190 dispatch, ≈12480–12550 bodies) they keep
`loaded / populated / dirty / read_only / row_key / pending_value / last_status`
in `BindingRuntimeState` — and nothing else. **`binding_update` checks
`read_only` and a row key, clears `dirty`, and returns.** It never touches the
file, the table or the connection. `binding_load` and `binding_populate` set
flags; the rows reach the grid by another route (§2.5).

### 2.4 Codegen already emits the surrounding COBOL
`cobolt-codegen/src/data_binding.rs`. Per binding: `01 <ID>-STATE` with
`-DIRTY`, `-READ-ONLY` (1 for a read-only binding), `-ROW-KEY PIC X(256)`,
`-STATUS PIC X(256)`; paragraphs `COBOL-DATA-BINDINGS-LOAD`, `-POPULATE`,
`-MARK-CLEAN` and **`-UPDATE`**, which calls `COBOL::"BINDING-UPDATE"` for each
**writable** binding only. Nothing performs `-UPDATE` today.

### 2.5 How rows get into a bound grid — and which sources can
`cobolt-form-host/src/seeding.rs` seeds `_Binding*` properties only for an
`IndexedFile` source (target `DataGrid`) and a `CobolTable` source; the
interpreter's `refresh_binding` serves exactly those two kinds.

- **Indexed** (`refresh_indexed_file_binding`, ≈13489): loads the `.cidx`,
  opens the data file through **`indexed_ide::GridSession`** and builds `Rows`
  from `session.rows()`, one tab-separated line per record, each field through
  `cobolt_indexed::format_field_display`.
- **Quick bind** — the `DataSource` property, `"indexed/actors.cidx /
  CUSTOMER-RECORD"`, set in the Properties pane — takes the same path
  (`populate_datasource_grids`, ≈13462) with no `DataBindingDef`: **no mode, no
  declared key.** PowerDemo3's Datagrid demo is bound this way (verified by
  running it: 1,098 actors loaded, `AllowCellEditing` unset).
- **COBOL table** (`refresh_datagrid_binding`, ≈12560): reads the table's
  cells from the environment by `subscript_key(field, [row])`.
- **SQL:** modelled, validated by the Guardian and labelled in the binding
  editor, but **no run-time load path exists** for a SQL source on a grid — no
  `_BindingKind` for it, no branch in `refresh_binding`. A program fills such a
  grid itself.

### 2.6 The Designer overwrites a bound grid on every frame
`designer.rs` ≈9616 calls `refresh_data_binding_target_properties`
(`app.rs` ≈20010), which for each `DataBindingDef` rewrites the target grid's
**`Columns`, `DataSource`, `Rows` and `AdvancedGrid`** from the binding — the
rows from a preview (WS `MOVE`s and `VALUE`s, else invented sample rows). The
merge keeps column *presentation* it can match by field — the COBOL mask, a
deliberately chosen Button/Gauge/Image kind, width, order — but the column set,
its titles and its cell values are the binding's, regenerated each frame. The
canvas itself draws static faces (`render_faces`); it takes no cell input. That
is why a bound grid cannot be edited visually in the Designer: what is typed is
rewritten the next frame, and nothing on the canvas takes the typing.

### 2.7 The grid already edits a cell and says so
`AllowCellEditing` (default off): double-click or F2 opens the cell in a text
box; Enter or clicking away keeps it, Escape drops it. A kept edit is written
into **`Rows`** and raises **`onCellEdited`**, with `EditedRow`, `EditedColumn`
(data row and column, from 1), `EditedValue`, `PreviousValue`; the handler
stores it, or refuses it with `SetCellValue(row, col, PreviousValue)`
(`render.rs` ≈10760–12140; Guide, *DataGrid*). **The grid only shows the new
text.** On a bound grid it is gone at the next `RefreshBinding`, which re-reads
the source.

### 2.8 The indexed engine already has a write path — in the IDE's own editor
`indexed_ide::GridSession` (`indexed_ide.rs` ≈178–300): `select_row(i)`,
`write_row`, `rewrite_row`, `delete_current`, **`commit`**, **`rollback`**; the
Indexed Editor's Apply button uses `select_row` + `rewrite_row`
(`panels/indexed_grid.rs` ≈247–262), building the record with
`cobolt_indexed::encode_field_display(field, text, len)`, which validates by
PICTURE and returns an error the editor shows. The engine's own `COMMIT` /
`ROLLBACK` are journaled (`indexed.rs`). This is the write path for an indexed
source; nothing new is needed below it.

### 2.9 What no form uses yet
No `.cfrm` in `examples/` or `tests/` carries a `<DataBindings>` block, and
none turns `AllowCellEditing` on. Nothing already shipped changes behaviour if write-back is
gated on a writable binding (R8).

## 3. Goals / Non-goals
- **Goals**
  - A DataGrid can be edited on the Designer canvas, bound or not (§4.1).
  - A bound grid with `AllowCellEditing` on writes a kept edit back to its
    source, per source kind (§4.3).
  - `AutoCommit` on writes it at once; off keeps it pending and lets the code
    confirm or discard it (§4.2).
  - Everything is built from §2's parts: the binding model, `BindingRuntimeState`,
    the `BINDING-…` built-ins, the generated `-UPDATE` paragraph, `onCellEdited`,
    `SetCellValue`, `GridSession`.
- **Non-goals**
  - No new binding model, no new storage engine, no new event loop, no new
    editor control: the in-cell editor is the one the grid already has.
  - No write-back for REST or Agent sources (they have their own update
    metadata, spec 022).
  - No adding or deleting rows from the grid in this spec — an edit of an
    existing cell (Q4).
  - No change to a grid that is not bound, or bound read-only, or has editing off.

## 4. Requirements (EARS)

### 4.1 Visual editing in the Designer
- **R1 (ubiquitous):** The Form Designer shall let the developer edit a
  DataGrid's presentation directly on the canvas: resize a column by dragging
  its edge, reorder by dragging its title, rename a title by double-clicking
  it, and set frozen columns. Width, order and frozen panes are operations the
  column-editor state layer (`data_grid_columns.rs`, `DataGridColumnEditorState`)
  already performs on `AdvancedGrid`; the title rename is new. They shall be
  reachable by direct manipulation as well as through *Edit DataGrid settings*.
- **R2 (state):** While the grid has **no** binding, the developer shall also be
  able to double-click a cell and edit its sample value, which is written to
  `Rows`; add and remove a column and a row.
- **R3 (state):** While the grid is **bound**, R1 shall hold and its edits shall
  persist: `refresh_data_binding_target_properties` shall preserve a column's
  title, width, order, frozen state, mask and control kind that the developer
  set. Today it already preserves width, order, mask and a deliberately chosen
  control kind; it **overwrites the title** with the field's display name every
  frame (`merge_datagrid_binding_column`), which R3 changes — a title the
  developer typed shall win over the binding's until it is cleared.
- **R4 (state):** While the grid is bound, its column **set** and the values of
  its cells shall be shown as the binding's, read-only on the canvas, and the
  canvas shall say why (a note, or a tooltip: "from <source>; edit the binding").
  Changing the set of fields is the Data Binding editor's job.
- **R5 (event):** Every canvas edit shall be an ordinary property change —
  undoable, saved in the `.cfrm` — as spec 088's grid edits are.
- **R6 (ubiquitous):** The canvas editor shall be the grid's existing in-cell
  editor, not a second one (§3).

### 4.2 Auto-commit and the decision
- **R7 (ubiquitous):** The DataGrid shall have an **`AutoCommit`** property, a
  boolean, default **on**, in *Edit DataGrid settings* beside *Allow cell
  editing*, in the Knowledge Base and in the Guide.
- **R8 (constraint):** Write-back shall happen only when **all** of these hold:
  the grid is bound; the binding is writable; `AllowCellEditing` is on. A grid
  that is not, behaves exactly as today.
- **R9 (event):** When `AutoCommit` is on and a kept edit raises
  `onCellEdited`, the runtime shall — after that handler returns — write the
  cell's *current* value to the source and commit it. A handler that restored
  `PreviousValue` has therefore refused the edit and nothing is written.
- **R10 (event):** When `AutoCommit` is **off**, a kept edit shall be recorded as
  the binding's pending edit (the `BINDING-SET-PENDING` state: row key, value,
  dirty) and `onCellEdited` shall be raised; nothing is written.
- **R11 (event):** While an edit is pending, the code shall confirm it with
  **`DG::CommitEdit()`** or discard it with **`DG::DiscardEdit()`**. Confirm is
  `BINDING-UPDATE` made real (R15); discard clears the pending state
  (`BINDING-MARK-CLEAN`) and puts `PreviousValue` back in the cell. Both return
  a status the code can test.
- **R12 (state):** A pending edit shall show in the grid as a changed cell (a
  marker the theme defines), so the operator can see what is not saved.
- **R13 (event):** When a write fails — a PICTURE the text does not fit, a
  record that changed or vanished, a lock, a constraint — the runtime shall put
  `PreviousValue` back, keep the failure in **`CommitStatus`** (the source's
  own: FILE STATUS, SQLSTATE) and raise **`onCommitError`** with it. The pending
  edit and row identity stay recoverable (Guide, §2.2).
- **R14 (event):** `RefreshBinding()` while edits are pending shall not discard
  them silently: it shall return the count it loaded and leave the pending
  edits and their cells as they are, or raise the same decision (Q3).

### 4.3 Writing to each source
- **R15 (ubiquitous):** `binding_update` shall do what its name says, by source
  kind, and report the source's own status in the binding's `-STATUS`:
  - **Indexed:** locate the record by its **key as loaded**, not by row
    position; encode the new text with `encode_field_display`; `rewrite_row`;
    `commit`; on error `rollback`. The key shall be the binding's `key_field`,
    and for a quick-bind grid the `.cidx`'s primary key (R16).
  - **COBOL table:** assign the cell of the bound WS table by
    `subscript_key(field, [row])`. A table has no commit; `AutoCommit` off still
    holds the edit pending (R10) and `CommitEdit()` assigns it.
  - **SQL:** issue an `UPDATE … SET <field> = :v WHERE <key fields> = :k` through
    the binding's `source_control_id` connection, and commit or roll back on it
    as `COBOL-EXEC-SQL` does. **A SQL source must first be loadable into a grid
    at run time (§2.5); that work belongs to this spec's plan.**
- **R16 (constraint):** A quick-bind grid has no `DataBindingDef`, so no mode:
  it shall be writable when `AllowCellEditing` is on and the `.cidx` declares a
  primary key — and the Properties pane shall say so beside the checkbox (Q2).
- **R17 (constraint):** A write shall identify the row by its key as of the load.
  If that key is no longer in the source, the write fails with the source's
  "not found" status (R13); it shall never write to the wrong record because the
  rows moved.
- **R18 (constraint):** The grid's edited column must map to exactly one source
  field. A column with no data behind it, or an image column, is not editable
  already (§2.7); a computed or formatted column is not either.
- **R19 (ubiquitous):** The numbers the handler sees — `EditedRow`,
  `EditedColumn`, from 1 — shall be unchanged; the source mapping shall be the
  runtime's.

### 4.4 Compatibility, parity, documentation
- **R20 (constraint):** No existing form shall change behaviour (§2.9): a grid
  that is read-only, unbound, or has `AllowCellEditing` off is untouched, and a
  handler that stores the edit itself in `onCellEdited` keeps working with
  `AutoCommit` off.
- **R21 (constraint):** The runtime change shall reach all three hosts that
  construct the interpreter (`rcrun run-form`, embedded child forms, and the
  compiled binary's `run_form_app`) — it lives in the interpreter, not in a host
  (`interpreter-binary-parity`).
- **R22 (ubiquitous):** The System KB tables (`cobolt-compiler`) shall carry
  `AutoCommit`, `CommitStatus`, `onCommitError`, `CommitEdit`, `DiscardEdit`, the
  completed `AllowCellEditing` text, and `chunked.data` shall be regenerated in
  the same change.
- **R23 (ubiquitous):** The Developer's Guide (*DataGrid*, *Data binding*) shall
  describe both modes with a COBOL example for each; its translations are
  deleted (Golden Rule 8).
- **R24 (ubiquitous):** Every new IDE string — the checkbox, the tooltip of R4,
  the quick-bind note of R16 — shall be a `Tr` field in all six languages.

## 5. Acceptance criteria
- [ ] AC1 — R1–R6: Designer tests: a column dragged to a new width and a title
  renamed survive a binding refresh; an unbound grid's cell edited on the
  canvas lands in `Rows` and undoes; a bound grid's canvas cells do not take
  input and say why.
- [ ] AC2 — R9, R13, R15 (indexed): a bound indexed grid, `AutoCommit` on: edit
  a cell, the file holds the new value after `rewrite_row` + `commit`, a
  re-read shows it; a handler that restores `PreviousValue` leaves the file
  unchanged; a text that does not fit the PICTURE restores the cell and sets
  `CommitStatus`.
- [ ] AC3 — R10, R11 (indexed): `AutoCommit` off: the file is unchanged after the
  edit; `CommitEdit()` writes it; `DiscardEdit()` restores the cell and leaves
  the file as it was; both return a status.
- [ ] AC4 — R15 (COBOL table): the WS cell holds the new value after confirm and
  the old value after discard.
- [ ] AC5 — R15 (SQL): against the in-memory SQLite connection used by the SQL
  tests, an edit becomes an `UPDATE … WHERE key`, commits, and a failing
  constraint rolls back and sets `CommitStatus`; a SQL source loads into a grid
  at run time.
- [ ] AC6 — R17: a record deleted by another process between load and edit
  yields `CommitStatus` "not found" and no write to a neighbour.
- [ ] AC7 — R8, R20: a grid that is read-only, unbound or has editing off, and
  an existing handler-stored edit, behave as in 1.91.14 (the existing
  `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed` and the
  runtime tests unchanged).
- [ ] AC8 — R21: the same edit tested through the interpreter that `rcrun
  run-form`, the child-form host and `run_form_app` each construct.
- [ ] AC9 — R22–R24: the KB freshness test green with a regenerated
  `chunked.data`; the Guide updated; six languages for every new `Tr` key.
- [ ] AC10 — Golden Rule 7: a `tests/cobol` program over a bound indexed grid
  prints the cases it ran (auto-commit on, off+confirm, off+discard, bad
  PICTURE, vanished record) and `TESTS RUN` / `TESTS FAILED` / `OVERALL RESULT`.
- [ ] AC11 — the PowerDemo3 Datagrid demo, with `AllowCellEditing` on, saves an
  edit of an actor's name into `actors.idx` and shows it after "Load data".

## 6. Constraints & steering check
- **i18n:** yes — R24.
- **Generated code:** codegen already emits `COBOL-DATA-BINDINGS-UPDATE` for a
  writable binding; it should *perform* the update on `onCellEdited` when
  `AutoCommit` is on only if the plan finds that the runtime route (R9) cannot
  do it — the runtime route is preferred, so generated COBOL and the interpreter
  do not each write.
- **System KB:** R22 — new properties, events and methods live in the Rust
  constant tables.
- **Three hosts:** R21.
- **Docs:** R23.
- **Fix vs feature:** **feature** (new capability). `features` branch, a `z` bump
  and a changelog entry; announced on f=96 only when the operator asks.
- **Window rule:** the Designer editor is the grid's existing editor in an
  existing canvas; nothing here sizes a window.

## 7. Plan outline (what the plan should decide, in this order)
1. **The runtime wire** (R9–R17), indexed first — everything it needs exists
   (§2.8): give `binding_update` its effect; call `BINDING-SET-PENDING`'s
   bookkeeping from the post-handler hook; add `CommitEdit` / `DiscardEdit` and
   the two properties/event. This is the part the demo (AC11) needs.
2. **COBOL table**, then **SQL** with its missing load path (§2.5).
3. **The Designer** (R1–R6): make `refresh_data_binding_target_properties`
   presentation-preserving; add the canvas gestures over
   `DataGridColumnEditorState`; add the read-only note.
4. KB, Guide, six-language strings, the `tests/cobol` program.

## 8. Open questions
- **Q1 — "edit visually in the Designer" — which of three things?** (a) the
  grid's *presentation* — column widths, order, titles, frozen columns (R1, R3);
  (b) its *sample values* — a cell typed on the canvas (R2, unbound only); (c)
  the **real records** of the bound source, edited in the Designer like the
  Indexed Editor does. This spec writes (a) for every grid and (b) for an unbound
  one, and keeps a bound grid's values read-only on the canvas (R4) because the
  Designer previews a binding with invented rows (§2.6) and writing real data
  from a design surface is a different, riskier feature. *Recommended: (a)+(b).
  If you meant (c), say so — it changes §4.1.*
- **Q2 — a quick-bind grid has no mode.** Writable when `AllowCellEditing` is on
  and the `.cidx` has a primary key (R16), or only through a `DataBindingDef`
  marked Writable? The first makes PowerDemo3's grid work with one checkbox;
  the second is stricter and needs the Data Binding editor. *Recommended: the
  first, with the note beside the checkbox.*
- **Q3 — `RefreshBinding()` with edits pending.** Keep them and return the count
  (R14, first option), or raise the decision, or discard with a status?
  *Recommended: keep, and return a non-zero `CommitStatus` so the code notices.*
- **Q4 — rows.** This spec edits existing cells. Adding and deleting rows in a
  bound grid (`AddRow`/`DeleteRow` already exist for `Rows`) is the same wire with
  `write_row` / `delete_current`; in or out? *Recommended: out, a follow-up.*
- **Q5 — names.** `AutoCommit`, `CommitStatus`, `onCommitError`, `CommitEdit()`,
  `DiscardEdit()` are proposals. The code's decision could instead use the
  built-ins directly (`COBOL::"BINDING-UPDATE"` to confirm,
  `COBOL::"BINDING-MARK-CLEAN"` to discard) and add no method — fewer names, but
  the discard would not restore the cell. *Recommended: the methods, as thin
  wrappers over the built-ins.*

## 9. Risks
- **Two writers.** The grid's session opens the data file; a running program may
  have it open. The indexed engine's locking and `FILE STATUS` apply (R13); the
  plan must say what `AutoCommit` does to a file another process holds.
- **Identity.** The grid shows sorted and filtered rows; the handler's
  `EditedRow` is the data row, the source key is the runtime's. R17 exists
  because a position-based write is the one way this feature could damage data.
- **Silent data change.** A checkbox that makes a grid write is a bigger
  decision than one that makes it edit; R8's three-way gate and R16's note exist
  for that reason.
- **Scope.** SQL needs a load path that does not exist (§2.5). If the plan finds
  it larger than the rest, SQL can ship as a second step behind the same
  contract.
