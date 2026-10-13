<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# DataGrid quality pass #2 — spec 095, milestone 4 (Dropdown cells, 1.91.24) and the key lock (1.91.25)

Run against `.agents/agents/datagrid-quality-compatibility-agent.md` and its skill
(`.agents/skills/datagrid-quality`). The agent is a checklist, not a program: this is the
checklist applied to the change by reading the diff and running the tests below. **No
pixel was looked at by a person** — the visual items are listed at the end for the operator.

## What changed in the DataGrid (areas affected)

| Area | Change |
|------|--------|
| Cell content (layer 4) | A Dropdown cell with a list paints the **text** of the item its value names (`datagrid::dropdown_text`); a value in no item, and a Dropdown with no list, paint as before. The box, the ▼ and the selection overlay are unchanged. |
| Interaction | A Dropdown cell with a list takes a click (opens its list), F2 and Alt+Down; it leaves the text editor. Its hit area is the part that can be seen (as the Checkbox's). While a list is open the grid's own arrow keys, Space and F2 stand down. A pointing-hand cursor over an actionable cell. The key cell of a loaded row takes no click and no editor. |
| The list | Not new painting: the ComboBox's own popup pass draws it (`OpenCombo` gained a `target`; a pick of a `GridCell` goes through `push_cell_edit`). |
| Selection / hover / focus (layer 6) | Unchanged paint. |
| Model | `DataGridColumn.dropdown` (`DataGridDropdown`), serde-defaulted, inside the `AdvancedGrid` JSON; `_ColumnItems`, `_RowOrigins`, `_KeyColumns` runtime properties. |

Not changed: padding, row/header height, grid lines, borders, corner radius, scrolling and
frozen-pane geometry, resize, the layer order of layers 1–3 and 7–8, the Checkbox cell, the Button,
Gauge and Image cells.

## The 18 regression scenarios

| # | Scenario | Evidence |
|---|----------|----------|
| 1 | Plain DataGrid, default theme | 056 corpus goldens unchanged (below) |
| 2 | Custom grid line colour / thickness | not touched; `cobolt-forms` suite green |
| 3 | Rounded corners | not touched; `a_child_at_a_rounded_corner_stays_inside_the_arc` green |
| 4 | Whole-grid background image | not touched |
| 5 | Whole-grid background pattern | not touched |
| 6 | Per-cell background colours | not touched; the failed mark still paints over them (`a_columns_own_background_does_not_hide_the_failed_mark`) |
| 7 | Per-cell background images | not touched |
| 8 | Alternating row colours | not touched |
| 9 | Selected row/cell styling | not touched; the Dropdown branch keeps its selection overlay |
| 10 | Hover and focus styling | no new paint; cursor icon only. A click on a Dropdown cell still selects it and raises `onCellClick` |
| 11 | Embedded TextBox cells | `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed` green; a Dropdown **without** a list is still edited as text (`a_dropdown_without_a_list_is_still_text_and_a_published_list_is_used`); the key cell of a loaded row is not (`the_key_of_a_loaded_row_is_read_only_and_a_new_rows_is_not`) |
| 12 | Embedded ComboBox cells (the Dropdown kind) | **now tested**: shows the text of its item; click opens; a pick stores the value and raises `onCellEdited`; the item the cell holds is no edit; Escape and a click outside change nothing; Up/Down/Enter; F2 and Alt+Down with type-ahead; double-click; disabled and editing-off do nothing and paint alike; a clipped row does not steal a click. The ComboBox control's own popup tests (`a_long_dropdown_scrolls…`, `the_arrow_keys_walk_a_combobox_open_or_closed`, `a_dropdowns_selection_band_is_cut_by_the_panels_corner`, …) are unchanged and green. |
| 13 | Embedded CheckBox cells | unchanged from M3; the same tests green |
| 14 | Active column resize | not touched |
| 15 | Active row resize | not touched |
| 16 | Many rows, virtual scrolling | not touched; the open list closes when its row is scrolled out of sight (`dropdown_header` is found while painting the visible rows) |
| 17 | Many columns, horizontal scrolling | the Dropdown's hit area is cell ∩ body clip, the rule the frozen-pane Checkbox test holds |
| 18 | Frozen headers / rows / columns | same |

Layer order (checklist §Validation): 1 grid background · 2 row background · 3 cell background · 4 cell
content · 5 custom control · 6 selection/hover/focus · 7 grid lines · 8 border — **unchanged**. The list
is drawn in the popup pass, above every control, not as a layer of the grid.

## Tests run (this tree, 2026-10-13)

- `cargo test -p cobolt-forms --features render --no-fail-fast`: **1,514 passed, 3 failed** (73 targets).
  The three predate the feature and fail identically without it: `xml::…every_example_form_round_trips_056`
  (75 example forms, expects 74), `xml::…no_example_form_gains_responsive_markup_on_save_056` (16, expects
  15) and `every_example_form_renders_as_its_golden` (stops at the Layers demo's missing snapshot).
- **The 056 corpus goldens** (PowerDemo3 and PowerChat, three surfaces, three scales) regenerated into the
  worktree with `COBOLT_WRITE_GOLDEN=1` on the final renderer and `git status` read: **no existing golden
  changed**; the only new file was the Layers demo's, another session's, and was removed again. No example has a
  Dropdown list or a key column, so this proves the Textbox / Button / Checkbox painting untouched, not that
  the Dropdown cell is right — that is the next lines.
- `cobolt-forms/tests/datagrid_cells_095.rs`: a Checkbox grid paints **30** and a Dropdown grid **34 identical
  shapes** on the running form (`render_form`), the preview (`render_form_with_chrome`) and the Designer's
  canvas (`render_form_scoped`); the item's text is painted, the value is not; another value, another
  picture; a value no item has shows as it is.
- Designer: `a_dropdown_cell_picks_on_the_canvas_and_undoes` — the list opens on the canvas, a pick writes the
  value into the sample `Rows` as one undo step, saves no run-time property.
- Teeth: the pick writing the item's text instead of its value fails `picking_an_item_stores_its_value…` and
  `a_dropdown_without_a_list_is_still_text…`; the grid's arrow keys not standing down fail
  `the_arrow_keys_and_enter_work_the_open_list`; the key lock taken out of the double-click fails
  `the_key_of_a_loaded_row_is_read_only…`.

## Findings

No regression found. Remaining risks, none of them blocking:

1. **A click outside the list closes it and then acts as it would.** The press closes the list; the click that
   ends on another cell selects it, and on a Checkbox cell toggles it. A ComboBox does the same to the control
   under its closing click. Deliberate; the Guide says a click elsewhere closes the list unchanged.
2. **The list is the shared popup pass.** That it opens in a window of its own when it overhangs the form is
   shared code, tested for the ComboBox (`a_dropdown_gets_a_window_only_when_it_overhangs`), not by a grid test.
3. **The Designer does not read a table or a file.** A Dropdown whose list is a COBOL table or an indexed file
   shows its stored value on the canvas (and is text-editable there); a static list works as at run time.
4. **A Dropdown column with an empty list is text.** Chosen so no form saved before this becomes uneditable; a
   source that momentarily gives no items makes the cell typeable until the next load.
5. **An Image cell can cover part of the 6×6 pending / failed mark** (carried from M3).

## For the operator to look at (never driven by an agent)

- A Dropdown column in Run Form on a themed grid (glass, a dark theme, rounded corners): the box, the ▼ and
  the item's text; a click opens a list in the same look as a ComboBox's; a hand cursor over it when editing
  is allowed, none when it is not.
- A list near the bottom of the form: it hangs below the cell and opens in a window of its own where it
  would overhang.
- The same grid on the Designer canvas in grid edit mode: a static list picks, a table list shows the code.
- A bound indexed grid with `AllowCellEditing` on: the key column cannot be edited on a loaded row, and can on
  a row the program added with `AddRow`.
