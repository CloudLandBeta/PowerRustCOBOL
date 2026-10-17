# Spec — DataGrid columns editor and row buttons

- **Status:** approved 2026-10-17. The operator decided Q1: exact column names, with clashes refused.
- **Folder:** specs/106-datagrid-columns-and-row-buttons/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-17
- **Classification:** fix (the operator's word, 2026-10-17). It completes what the DataGrid's own surfaces already half-offer: Button columns exist but cannot be given a caption, and the settings window cannot add, delete or order a column.

## 1. Overview

The operator, 2026-10-17, with a screenshot of *Edit DataGrid settings*:

1. Reorganise the interface: the columns on the left and the selected column's details on the right, with a vertical scroll so the window does not grow.
2. Buttons to add and delete columns.
3. Column content: from the data binding, or a **button** that stands for an action. The button can show text, a system icon, or both. Clicking it raises a DataGrid event, `onRowButtonClick`. Its LINKAGE SECTION receives a group: level 05 holds the row number and the name of the button that was clicked; a level-05 sub-group holds one level-10 item per column of the row:

   ```cobol
   01 DATAGRID-ROW-CONTENT.
      05 ROW-NUMBER ...
      05 ROW-NAME ...
      05 ROW-COLUMNS.
         10 <column 1 name> ...
         10 <column 2 name> ...
   ```
4. Reorder the columns by drag and drop, as well as with move-up and move-down buttons.

## 2. What exists today

*Mapped 2026-10-17 at d10d7d8b.*

**The settings window** (`properties.rs`, `show_datagrid_editor_modal`):
- It is one `resizable(true)` window holding one vertical scroll area.
- Each column is a collapsible header, with about 25 fields inside.
- It has no add, delete or reorder.
- The window is `resizable(true)` around a scroll area, which breaks CONVENTIONS' golden rule that a window never resizes itself.

**Columns.** There are two descriptions of a grid's columns:
- `Columns` holds the data schema: one `name:type` line per cell of each `Rows` line.
- `AdvancedGrid` (JSON, `DataGridAdvanced`) holds the display order and presentation. Each of its columns is matched to a `Columns` line by name.
- The canvas edit mode (spec 094, `grid_edit.rs`) already has `add_column`, `remove_column` and `move_column`, which keep the two in step.

**Button columns** already exist:
- A column whose edit control is `Button` draws its cell **value** as a caption, or `icon:<name>` as an icon.
- A click fires `onCellClick` with `ClickedRow` and `ClickedColumn`.
- There is no caption or icon *on the column*.
- A column with no data behind it draws an empty button.
- Re-applying a data binding **drops** any column with no bound field behind it.

**Event payloads.** Exactly two events deliver data to a handler:
- `CONTROL-ARRAY-INDEX`, a real `CALL … USING`;
- `CONTROL-NODE-DATA`, bound by name.

`event_linkage(event)` builds a handler's LINKAGE from the event name alone. A handler's skeleton is written once, then belongs to the developer.

**BY REFERENCE.** The runtime already lays a callee's LINKAGE group over the caller's bytes, field by field. NIST IC exercises this, and it includes a group laid over an elementary argument (a *record view*).

## 3. Goals / Non-goals

**Goals:**
- the two-pane columns editor;
- add, delete and reorder, each one undo step;
- a column whose content is a button, with a caption, an icon or both;
- `onRowButtonClick` with `DATAGRID-ROW-CONTENT`, in the Designer's Run Form, child forms and the compiled binary alike.

**Non-goals:**
- New cell kinds.
- Changing `onCellClick`: it still fires for every cell, buttons included (spec 095 R18 stands).
- A visual redesign of the other sections of the window.

## 4. Requirements (EARS)

### The window

- **R1** The *Edit DataGrid settings* window shall follow CONVENTIONS' window rule:
  - it has a stored size, seeded from constants;
  - it uses `.resizable(false).fixed_size(size)`;
  - it has one grip, which is the only thing that changes the size;
  - every child is laid out from that stored size.

  Its sections and their order stay as they are.
- **R2** The Columns section shall be **two panes** of a fixed height:
  - **Left:** the list of columns, as "*n* — title", with its own scroll.
  - **Right:** the details of the one selected column, with its own vertical scroll.
  - The window's height shall not change when a different column is selected or a column's details grow.
  - Selecting a column in the list shall show its details.
  - With no column, the right pane shall say how to create one.
- **R3** The COLUMNS header shall carry **Add (+)**, **Delete (−)**, **Move up (▲)** and **Move down (▼)**, each with a tooltip.
  - **Delete** and the moves act on the selected column, and are disabled with a reason when they cannot act.

### Editing columns

- **R4** **Add** shall add a column after the selected one (or at the end). `Columns`, `Rows` and `AdvancedGrid` stay consistent, using the same code as the canvas edit mode. The new column is selected.
- **R5** **Delete** shall ask for confirmation in the window, naming the column. Then it removes the column from `Columns`, from every row's cells and from `AdvancedGrid`, using the canvas edit mode's code. *(User code is sacred: only an explicit, confirmed delete.)*
- **R6** **Move up / Move down**, and **dragging a column in the list** onto another position, shall change the display order.
  - The drag shows an insertion line, as the Tab Order list does.
  - The data schema and the rows keep their order, as on the canvas.
- **R7** Each of R4–R6 shall be **one undo step** in the Form Designer.

### Button columns

- **R8** Each column's details shall start with **Content**:
  - **Data**: what the binding or `Columns` gives, as today.
  - **Button**: an action.
- **R9** A Button column shall carry a **caption** (text, may be empty) and an **icon** (one name from the system icon catalogue, chosen with the IDE's icon picker, may be none). At least one of the two shall be set; the window says so while neither is.
- **R10** Every row shall draw the Button column as its caption and/or icon:
  - in the Designer, in Run Form, in child forms and in the compiled binary;
  - with the existing Button cell's look and hover.

  A Button column with neither caption nor icon keeps today's behaviour: the cell value as caption, or `icon:<name>`.
- **R11** Re-applying a data binding shall **keep** Button columns with no field behind them, in their place.
- **R12** The two new column fields shall be stored so that an older `.cfrm` loads unchanged and a form with no Button caption or icon saves exactly as it did.

### The event

- **R13** A click on a Button cell shall raise **`onRowButtonClick`** on the grid, in addition to `onCellClick`.
  - The event shall be listed for the DataGrid wherever events are listed: the Events tab, completion, the agent context, the KB and the plugin reference.
- **R14** A handler for `onRowButtonClick` shall receive, **by reference**:

  ```cobol
  01 DATAGRID-ROW-CONTENT.
     05 ROW-NUMBER   PIC 9(9).      *> the row's number in Rows, 1-based (survives sorting)
     05 ROW-NAME     PIC X(30).     *> the clicked button's column name
     05 ROW-COLUMNS.
        10 <name>    PIC <mask>.    *> one per column, in display order, Button columns excluded
  ```

  - Each `<name>` is the column's COBOL name, exactly as the developer sees it (operator, Q1).
  - Each `<mask>` is the column's COBOL mask, or `X(30)` when it has none.
  - Each item holds that row's cell for the column, converted to its mask.
- **R15** The group's description shall never go stale.
  - The handler skeleton's LINKAGE SECTION holds the one line `COPY DATAGRID-ROW-CONTENT.`.
  - **Every generation replaces that line with the group**, written from the grid's columns as they are at that moment (`cobolt_forms::row_content`). Renaming, adding or removing a column therefore changes the handler's view on the next regenerate, without touching the developer's code.
  - The generated program also seeds the grid with the layout it was generated with (the hidden property `_RowContent`). The runtime packs the row by that seed, never by the columns as they stand at run time, because the user may reorder or retitle columns while the form runs.
  - *Changed during implementation (2026-10-17):* the first draft wrote a copybook file per grid (`COPYBOOKS/<form>-<grid>-ROW.cpy`). Expanding the line in the generated source gives the same never-stale view with no file to keep in step. It also keeps the debugger's line map exact, because each line the developer wrote maps to itself and the group maps to nothing.
- **R16** The generated program shall pass the group to the handler with `CALL … USING`, from a buffer the main program declares only when some grid binds `onRowButtonClick`.
  - The runtime shall fill that buffer from the clicked row before the call.
  - This reaches the three hosts through the shared renderer and the interpreter, with no host edit.
- **R17** `Check` shall report:
  - a Button column with neither caption nor icon, as a **warning** (it still shows each row's own cell value, R10);
  - a column whose name cannot be a COBOL word;
  - a column whose name is also a data item of the form, a reserved word, or another level-10 name of the same grid.

  The settings window shall show the same warning beside the column's name as it is typed (Q1).

### Fit

- **R18** Every new string shall be a `Tr` field in the six languages.
- **R19** Documentation:
  - The Developer's Guide (English only, GOLDEN RULE #8) and the compiler KB (with `chunked.data` regenerated) shall describe the Content choice, the event and its group.
  - The generated-code contract shall be recorded in the codegen goldens.
- **R20** The DataGrid Quality & Compatibility review shall run before completion. It covers its 18 scenarios, plus a Button column in a grid that is sorted, filtered and scrolled.

## 5. Acceptance criteria

- [ ] AC1:
  - The window does not change height when a column with many details and one with few are selected in turn, over 120 frames, in six languages.
  - The window resizes only by its grip (R1, R2).
- [ ] AC2 — Add, Delete (after confirming) and Move each change `Columns`, `Rows` and `AdvancedGrid` exactly as the canvas edit mode does, and each is undone by one Undo (R4–R7).
- [ ] AC3 — Dragging column 3 above column 1 in the list gives the order 3, 1, 2 (R6).
- [ ] AC4 — A Button column with a caption, an icon, or both paints them in the designer face and the run-form renderer, and an old form round-trips byte for byte (R10, R12).
- [ ] AC5 — Re-applying a binding keeps a Button column with no field behind it (R11).
- [ ] AC6 — Clicking the button on the second row of a sorted grid raises `onRowButtonClick`; the handler reads ROW-NUMBER = the row in `Rows`, ROW-NAME = the button column, and each column's value (R13, R14).
- [ ] AC7 — The same click works in `rcrun run-form`, a child form and a compiled binary (R16).
- [ ] AC8:
  - Renaming a column and regenerating changes the group written in place of the `COPY` line.
  - The handler that uses the new name compiles; the old name is reported by `Check`.
  - The developer's handler text is untouched (R15).
- [ ] AC9 — A form with no `onRowButtonClick` generates the same COBOL as before (golden unchanged) (R16).
- [ ] AC10 — The six-language string test and the KB freshness test pass (R18, R19).

## 6. Constraints & steering check

- **i18n:** yes, six languages.
- **Generated-code contract:**
  - New, for a form where a grid binds the event: the group in place of the handler's `COPY DATAGRID-ROW-CONTENT.` line, a buffer in the main program, the `_RowContent` seed in COBOL-MAIN, and `CALL … USING` in the dispatcher for that event.
  - Unchanged when the event is not bound (AC9).
- **`.cfrm`:** two optional column fields in the `AdvancedGrid` JSON; nothing else.
- **Docs:** the Guide's DataGrid section and the event list; the compiler KB.
- **Window rule:** the window is brought under it (R1).

## 7. Risks

- **Name clashes in the run unit.** The runtime keeps one store per run, and a nested program's names are added only when they are new. A level-10 name equal to a data item of the form would read or write that item. This is prevented by refusing such names (R17).
- **Mask sizes.** A column without a mask gets `X(30)`. A longer value is truncated in the group, never in the grid.
- **Sorting.** ROW-NUMBER must be the row in `Rows`, not the row on screen. The renderer already reports the data row, but this needs a test (AC6).

## 8. Decisions

- **Q1 — The level-10 names** (operator, 2026-10-17): **exactly the column names**. A clash is made impossible rather than renamed away: `Check` refuses a column name that is also a data item of the form, a reserved word, or a duplicate within the grid, and the settings window warns as the name is typed (R17).
  - Prefixed names (`RC-<column>`) were rejected because they are not the names the developer sees.
- **Where the work lives:** branch `fixes-datagrid-columns`, off `main`, in this worktree once the Task Wizard is committed.
