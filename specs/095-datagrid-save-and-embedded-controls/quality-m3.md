<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# DataGrid quality pass #1 — spec 095, milestone 3 (Checkbox cells, 1.91.23)

Run against `.agents/agents/datagrid-quality-compatibility-agent.md` and its skill
(`.agents/skills/datagrid-quality`). The agent is a checklist, not a program: this is the
checklist applied to the change by reading the diff and running the tests below. **No
pixel was looked at by a person** — the visual items are listed at the end for the operator.

## What changed in the DataGrid (areas affected)

| Area | Change |
|------|--------|
| Cell content (layer 4) | A Checkbox cell's tick is decided by `datagrid::checkbox_checked` (was an inline word list). Same words paint the same tick when the column sets no pair of its own. |
| Interaction | A Checkbox cell takes a click, and Space on the selected one; it leaves the text editor. Its hit area is the part that can be seen, and no longer depends on `SelectableText`. A pointing-hand cursor over an actionable cell. |
| Selection / hover / focus (layer 6) | Unchanged paint. The selection overlay branch for a Checkbox cell is the one it already had. |
| Model | `DataGridColumn.checked_value` / `unchecked_value` (serde-defaulted, inside the `AdvancedGrid` JSON). |
| Marks (M1, re-checked) | The pending / failed mark is painted after the background layers, under the content. |
| Binding merge | `merge_datagrid_binding_column` keeps the two new fields. |

Not changed: padding, row/header height, grid lines, borders, corner radius, scrolling and
frozen-pane geometry, resize, the layer order of layers 1–3 and 7–8.

## The 18 regression scenarios

| # | Scenario | Evidence |
|---|----------|----------|
| 1 | Plain DataGrid, default theme | no painting code touched on this path; 056 corpus goldens unchanged (below) |
| 2 | Custom grid line colour / thickness | not touched; `cobolt-forms` suite green |
| 3 | Rounded corners | not touched; `a_child_at_a_rounded_corner_stays_inside_the_arc` green |
| 4 | Whole-grid background image | not touched |
| 5 | Whole-grid background pattern | not touched |
| 6 | Per-cell background colours | the mark now paints **over** them: `a_columns_own_background_does_not_hide_the_failed_mark` |
| 7 | Per-cell background images | not touched (the mark's new place is after them) |
| 8 | Alternating row colours | not touched |
| 9 | Selected row/cell styling | not touched; the Checkbox branch keeps its selection overlay |
| 10 | Hover and focus styling | no new paint; cursor icon only. A click on a Checkbox cell still selects it and raises `onCellClick` (`a_click_on_a_checkbox_cell_toggles_it_and_raises_on_cell_edited`) |
| 11 | Embedded TextBox cells | `a_datagrid_cell_is_edited_in_place_when_editing_is_allowed` green (the text editor now ends through `push_cell_edit`) |
| 12 | Embedded ComboBox cells (the Dropdown kind) | **unchanged in M3** (still painted, still text-editable); no test exists today — M4 gives it one |
| 13 | Embedded CheckBox cells | new: click toggles; Space toggles; the column's own pair is written and drawn; empty cell checks; disabled / editing-off does nothing and paints alike (R17); double-click is two toggles and opens no text box; works with `SelectableText` off; Button cells unchanged (R18) |
| 14 | Active column resize | not touched |
| 15 | Active row resize | not touched |
| 16 | Many rows, virtual scrolling | not touched; the hit area uses the body clip, so a row cut by the body edge cannot take a control below's click — `a_clipped_checkbox_row_does_not_steal_the_click_of_a_control_below` |
| 17 | Many columns, horizontal scrolling | `a_scrolled_checkbox_under_the_frozen_band_does_not_steal_the_frozen_cells_click` (a real wheel scroll) |
| 18 | Frozen headers / rows / columns | same test; layout tests `datagrid_layout_*_023` green |

Layer order (checklist §Validation): 1 grid background · 2 row background · 3 cell background · 4 cell
content · 5 custom control · 6 selection/hover/focus · 7 grid lines · 8 border — **unchanged** by this
milestone; the M1 mark moved from before layer 3 to between layers 3 and 4.

## Tests run (this tree, 2026-10-13)

- `cargo test -p cobolt-forms --features render --no-fail-fast`: **1,497 passed, 3 failed.** The three
  predate the feature and fail identically without it: `xml::…every_example_form_round_trips_056`
  (75 example forms, expects 74), `xml::…no_example_form_gains_responsive_markup_on_save_056` (16, expects
  15) and `every_example_form_renders_as_its_golden` (stops at the Layers demo's missing snapshot).
- **The 056 corpus goldens** (PowerDemo3 and PowerChat, three surfaces, three scales) were regenerated
  into the worktree with `COBOLT_WRITE_GOLDEN=1` and `git status` read: **no existing golden changed**; the
  only new file was the Layers demo's, which is another session's, and was removed again. No example has a
  Checkbox column, so this proves the Textbox / Button painting is untouched, not that the Checkbox cell is
  right — that is the next line.
- `cobolt-forms/tests/datagrid_cells_095.rs`: a grid with a Checkbox column paints **30 identical shapes** on
  the running form (`render_form`), the preview (`render_form_with_chrome`) and the Designer's canvas
  (`render_form_scoped`); flipping the values changes the picture; `yes`/`no`/`false` draw as `Y`/`N`/`N`.
- Designer: `a_checkbox_cell_toggles_on_the_canvas_and_undoes` — in grid edit mode a toggle is one undo step
  and saves no run-time property.
- Teeth: both click-area tests were run with the clipped hit area put back to the whole cell — **both fail**
  (the cut-off row toggled; the frozen cell's click went to the checkbox under it) — and pass with it.

## Findings

No regression found. Remaining risks, none of them blocking:

1. **An Image cell can cover part of the 6×6 pending / failed mark** (the picture is drawn inside the cell,
   after the mark). Pre-existing for the amber mark; the mark is still above every background.
2. **The Dropdown kind is still text-editable and has no test (scenario 12)** until M4 — by design, so the
   Dropdown does not become uneditable between two milestones.
3. **`X`/space pair.** A blank cell is read as empty (so it starts from `Y`/`N`); the `X`/space pair is
   reached from a blank only when the column names `X` as its Checked value. Documented in the Guide.
4. **A double-click is two toggles.** Deliberate (a checkbox control behaves so); the Guide says it.

## For the operator to look at (never driven by an agent)

- A Checkbox column in Run Form on a themed grid (glass, a dark theme, rounded corners): the box and tick
  as before, a hand cursor over it when editing is allowed, none when it is not.
- The same grid on the Designer canvas in grid edit mode: clicking a box toggles the sample data.
- A grid with a coloured column background and a pending edit: the amber mark visible on top of the colour.
