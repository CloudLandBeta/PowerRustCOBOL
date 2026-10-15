# Spec — Properties that go together are drawn one after the other

- **Status:** implemented in 1.91.165 (2026-10-15)
- **Folder:** specs/102-property-order/
- **Author:** Anthropic Claude Codex Agent   **Date:** 2026-10-15
- **Origin:** the operator, 2026-10-15 — *"reorder the properties so properties
  that go together (for example, but not limited to: gradient start color / end
  color) are placed one after the other. Apply to all properties of all
  controls."*

## 1. What this is

The Properties pane draws a control's rows in the order its code happens to
list them. Over a hundred changes by many hands left related properties apart:
a chart's `ShowXAxis` and `XAxisLabel` five rows from each other, a check
box's tick split from its box by a section header, `ScrollBars` away from
`Multiline`. This changes the **order** the pane draws rows in — only the
order. No property, row, label, default or saved value changes.

## 2. Assumptions (stated, so they can be corrected)

1. "Properties" means the rows of the Properties pane's **Visuals** tab, for
   every control type in `ControlType::ALL` — 62 at the time of writing.
2. The **form's own** pane is not a control and is left as it is.
3. A *family* is the set of properties a developer thinks of as one thing. A
   family that already sits inside one section stays in that section; a family
   split by a section header is brought into one section.
4. A property that a control draws **twice** (ToolBar and Viewer repeat a few
   of the standard rows in their own sections) is a duplicate, not a split, and
   is reported rather than removed.

## 3. Requirements

- **R1.** Within a control's Properties pane, the members of a family that the
  pane shows are drawn one after the other: no other row and no section header
  between two of them.
- **R2.** A gradient's start colour is immediately followed by its end colour,
  for every gradient (`Background…`, `Border…`, `Caption…`).
- **R3.** `GridColumn` is followed by `ColumnSpan`, `GridRow` by `RowSpan`,
  `MinWidth` by `MaxWidth` and `MinHeight` by `MaxHeight`.
- **R4.** The rule holds in every state a control can be drawn in: as created,
  with every boolean property on, with every one off, and — for the Layout
  section — on a responsive form in each of the four layout modes.
- **R5.** No row is added, removed, renamed or re-labelled; no property default,
  range or stored value changes.

## 4. Acceptance criteria

- [x] **AC1** — `properties_that_go_together_are_drawn_one_after_the_other`
      (cobolt-ide, `panels/properties.rs`) reads the rows the pane really
      draws, for every control type in 3 states + 4 layout modes, and finds no
      family split. It was written failing first: **73 split reports** across
      17 control types on its first run (two of my families overlapped and were
      merged afterwards, so that count is an upper bound), **0** after.
- [x] **AC2** — the test is effective on the layout rows: reverting the
      Grid and Min/Max orders alone makes it report 380 splits.
- [x] **AC3** — the 67 other Properties-pane tests (help text for every row,
      search, six-language coverage) pass unchanged.
- [x] **AC4** — the rows before and after hold the same set: every line removed
      from the pane's source is re-added elsewhere except ten that are replaced
      comments or a merged `if` (checked by comparing the diff's removed and
      added lines).

## 5. What the families are

The table is `FAMILIES` in the test, one named list per group — the executable
form of this spec. In short: background and border (with their gradients and
glow); font; each control's icon, image, check-mark, caption, tab, list-item,
highlight, scroll and expand groups; a tree's lines, hot tracking, selection,
icons and metrics; a slider's range and ticks; a chart's title, axes,
animation, monochrome and points; a gauge's levels; a Shape's fill and line; a
Line's direction and style; a radar's scale and series; a snackbar's stack; an
agent's generation, tools and output; a REST client's request, auth and
transport; a viewer's bubbles; the responsive layout's grid cell, flex item,
size limits, font scaling, padding, container and gaps.

## 6. Left as found, on purpose

- **ToolBar** draws `BackgroundColor`, `Transparency` and `CornerRadius` a
  second time in its own frame (1.61.69) with the bar's own defaults, which the
  standard rows do not show; with a gradient on, the standard `BackgroundColor`
  row is hidden and the bar's own stays. Listed in `KEPT_APART`.
- **Viewer** draws `FontSize` in *Layout & view* as well as in *Appearance*.
- **The form's pane** — its *Appearance* card puts `Transparency`, the grid and
  the theme between the background colour and the background image.
- `CornerRadius` stays with *Geometry*, where it has always been, though it is a
  border property.

## 7. Steering compliance

- [x] i18n: no string added, changed or moved — only the order rows are drawn in.
- [x] Generated-code contract untouched.
- [x] English Developer's Guide: the Properties-pane paragraph says related
      properties sit together. No translation exists to delete.
- [x] System KB: the AWS Polly seeds were reordered (`VoiceId`, `Engine`,
      `OutputFormat`, `OutputFile`), which the published control documentation
      reads, so `chunked.data` was regenerated.
- [x] Feature → `features` branch, `z` bump, CHANGELOG entry.
