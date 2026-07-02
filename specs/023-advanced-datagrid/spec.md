# Spec — Advanced DataGrid

- **Status:** draft → approved
- **Folder:** specs/023-advanced-datagrid/
- **Author:** Codex   **Date:** 2026-06-30

## 1. Overview

PowerRustCOBOL's DataGrid needs to grow from a simple bound-table renderer into
a production RAD grid suitable for large COBOL table, indexed-file, SQL, and
REST datasets. The advanced DataGrid must keep existing `.cfrm` and
data-binding behavior working while adding fast virtual scrolling, richer
column/row interaction, filtering, freezing, CSV export, selectable text, and
cell-level presentation features such as framed status pills and numeric
gauges.

## 2. Goals / Non-goals

- **Goals:**
  - Support large row counts through virtual scrolling without drawing rows
    outside the grid bounds.
  - Allow users to resize columns and rows at design time and runtime where the
    grid is interactive.
  - Provide column reorder controls, chained per-column filters, freeze panes,
    CSV export, selectable/copyable cell text, and configurable grid-line style.
  - Support per-column formatting for foreground/background colors, framed cell
    content, corner radius, and optional numeric gauges.
  - Ensure font properties applied in the Form Designer are honored by the
    runtime grid.
  - Preserve existing DataGrid properties, data binding settings, generated
    COBOL behavior, and `.cfrm` load/save compatibility.
- **Non-goals:**
  - Do not replace the DataGrid's public `.cfrm` model with an incompatible
    file format.
  - Do not hand-edit generated COBOL as part of this feature.
  - Do not require users to learn Rust or configure a Rust-only grid API.
  - Do not implement spreadsheet formulas, cell merge/split behavior, or full
    Excel-compatible editing.

## 3. User stories

- As a COBOL RAD developer, I want a bound DataGrid to handle many rows smoothly,
  so that COBOL tables and data sources can be browsed without sluggish redraws.
- As a form designer, I want to resize columns and rows visually, so that the
  grid layout fits business data without manual property editing.
- As an end user, I want to filter one or more columns, so that I can narrow
  large datasets using chained conditions.
- As a form designer, I want status values to render as colored rounded pills,
  so that business state is legible at a glance.
- As an end user, I want to select and copy cell text, so that I can reuse grid
  data outside the application.
- As a business user, I want to export grid content to CSV, so that grid data can
  be shared with spreadsheets and external systems.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The system shall render DataGrid content inside the grid
  bounds and shall never draw row or cell content outside the DataGrid control
  rectangle.
- **R2 (ubiquitous):** The system shall support virtual scrolling so that the
  DataGrid renders only visible rows plus a small buffer, regardless of the
  total row count.
- **R3 (state):** While the DataGrid has more rows than can fit in its viewport,
  the system shall provide fast vertical scrolling and maintain a stable header.
- **R4 (state):** While the DataGrid has more columns than can fit in its
  viewport, the system shall provide horizontal scrolling without losing frozen
  columns.
- **R5 (event):** When a user drags a column edge, the system shall resize that
  column and persist the new width in the form model.
- **R6 (event):** When a user drags a row edge or changes the row-height
  property, the system shall resize the affected row or default row height and
  persist the result according to the configured row sizing mode.
- **R7 (ubiquitous):** The system shall expose column reorder controls in the
  DataGrid header, including explicit move-up/move-down or move-left/move-right
  icons appropriate to the current header layout.
- **R8 (event):** When a user activates a column reorder icon, the system shall
  move that column in the display order without losing its binding, formatting,
  filter, width, or sort metadata.
- **R9 (ubiquitous):** The system shall provide a filter input for each
  filter-enabled column.
- **R10 (state):** While multiple column filters are active, the system shall
  apply them as chained filters and show only rows that satisfy all active
  filter predicates.
- **R11 (ubiquitous):** The system shall expose per-column formatting settings
  for foreground color, background color, text alignment, and optional framed
  content.
- **R12 (optional):** Where framed content is enabled for a column, the system
  shall render the cell value inside a rounded frame whose background,
  foreground, padding, and corner radius can be configured.
- **R13 (optional):** Where value-to-style rules are configured for a column, the
  system shall apply rule-specific foreground/background/frame colors based on
  the cell value, such as rendering `Active`, `Trial`, `Churned`, and
  `Suspended` with different pill colors.
- **R14 (ubiquitous):** The system shall apply DataGrid font properties from the
  Form Designer to the runtime grid, including font family, size, bold, italic,
  underline, and foreground color where applicable.
- **R15 (optional):** Where a numeric gauge is enabled for a numeric column, the
  system shall render a proportional gauge behind or alongside the numeric value
  without hiding the text.
- **R16 (ubiquitous):** The system shall expose properties and methods to freeze
  a configurable number of leading columns and top rows.
- **R17 (state):** While columns or rows are frozen, the system shall keep frozen
  columns and rows visible during scrolling and shall align them with the
  scrollable body cells.
- **R18 (ubiquitous):** The system shall provide a visible CSV export command
  for grids where CSV export is enabled.
- **R19 (event):** When the CSV export command is triggered, the system shall
  export the currently configured grid data to CSV using the grid's delimiter,
  visible-column order, and filter mode.
- **R20 (ubiquitous):** The system shall allow users to select text within a cell
  and copy the selected text to the clipboard.
- **R21 (event):** When no partial text selection exists and a copy command is
  triggered, the system shall copy the selected cell, row, or range according to
  the active selection mode.
- **R22 (ubiquitous):** The system shall expose grid-line style settings with at
  least `Solid`, `Dash`, `Dots`, and `None`.
- **R23 (state):** While grid-line style is `None`, the system shall not draw
  internal grid lines but shall preserve readable cell spacing.
- **R24 (constraint):** The system shall preserve existing DataGrid properties,
  binding targets, `Rows`/`Columns` behavior, and `.cfrm` round-trip
  compatibility.
- **R25 (constraint):** The system shall keep DataGrid rendering consistent
  across Form Designer, live Preview, Run Form interpreter, and compiled binary
  surfaces through the unified rendering architecture.
- **R26 (constraint):** If an external grid engine such as the MIT-licensed
  `uiGrid` is adopted, the system shall wrap it behind PowerRustCOBOL's existing
  DataGrid model and shall not expose third-party API details to COBOL users or
  `.cfrm` files.
- **R27 (constraint):** The system shall keep DataGrid behavior accessible from
  RustCOBOL via properties and methods rather than Rust-only callbacks.
- **R28 (constraint):** The system shall not degrade data-bound control behavior
  guarded by the data-binding guardian, including COBOL table, indexed-file,
  SQL, REST API, and future Agent AI sources.

## 5. Acceptance criteria

- [ ] AC1 — A grid with at least 100,000 rows renders only the visible row range
  plus a bounded buffer and remains responsive while scrolling.
- [ ] AC2 — Rows beyond the visible viewport are clipped and never appear outside
  the DataGrid rectangle.
- [ ] AC3 — Vertical scrolling keeps the header fixed; horizontal scrolling keeps
  frozen columns fixed when configured.
- [ ] AC4 — Column edge drag changes column width and the width survives save,
  reload, preview, and run.
- [ ] AC5 — Row edge drag or row-height property changes affect row height and
  survive save, reload, preview, and run.
- [ ] AC6 — Column reorder icons move columns while preserving binding,
  formatting, filter, width, sort, and frozen-state metadata.
- [ ] AC7 — Two or more active column filters are applied as an AND chain, and
  clearing one filter immediately expands the result set accordingly.
- [ ] AC8 — A status column can render `Active`, `Trial`, `Churned`, and
  `Suspended` as rounded colored pills matching per-value style rules.
- [ ] AC9 — Grid font settings configured in the Form Designer are visible in
  Run Form and compiled output.
- [ ] AC10 — A numeric column can show a gauge and still display selectable text.
- [ ] AC11 — Frozen leading columns and top rows remain visible and aligned while
  the user scrolls.
- [ ] AC12 — The CSV export button appears only when CSV export is enabled and
  exports columns in the displayed order.
- [ ] AC13 — Users can select text inside a cell and copy it to the clipboard.
- [ ] AC14 — Copying with no partial text selection copies the selected cell,
  row, or range according to `SelectionMode`.
- [ ] AC15 — `Solid`, `Dash`, `Dots`, and `None` grid-line styles render
  distinctly in designer, preview, run, and compiled surfaces.
- [ ] AC16 — Existing forms using simple DataGrids load and render without
  manual migration.
- [ ] AC17 — Data-bound grids continue to receive rows from existing COBOL table,
  indexed-file, SQL, and REST binding configurations.
- [ ] AC18 — New user-facing labels and tooltips are localized in EN, ES, PT, JA,
  ZH, and FR.
- [ ] AC19 — The English developer guide documents the advanced DataGrid
  properties, methods, and CSV/filter behavior.

## 6. Constraints & steering check

- **i18n (6 languages) impact?** Yes. Header controls, property labels, filter
  placeholders, CSV export labels, tooltips, validation messages, and any new
  DataGrid dialogs must use `Tr` fields translated in EN/ES/PT/JA/ZH/FR.
- **Generated-code / regenerate contract impact?** Yes. Any generated COBOL that
  initializes advanced DataGrid properties or calls export/filter/freeze methods
  must be emitted by `cobolt-codegen`, start with the standard banner, remain
  parseable, and be regenerated on Build / Run / Debug / Check.
- **Docs (English guide) update needed?** Yes. Update only
  `docs/developers-guide-en.md`; translated guides remain user-maintained.
- **Fix vs feature classification:** Feature specification. Implementation should
  follow the project's current versioning convention when executed.
- **Unified renderer impact:** High. The feature must preserve parity across
  `render::render_form` and `render::render_faces`.
- **Data-binding guardian impact:** High. Data-bound grids must not lose binding
  metadata when columns are resized, reordered, filtered, frozen, styled, or
  exported.
- **Third-party dependency impact:** Open for `/plan`. `uiGrid` is reported MIT
  licensed by the user, but the plan must still verify repository/package shape,
  Rust version compatibility, egui compatibility, maintenance risk, and whether
  it can serve static designer rendering as well as interactive runtime rendering.

## 7. Open questions

- Q1: Should CSV export include all data rows or only currently filtered rows by
  default?
- Q2: Should row resizing be per-row, global default row height, or both?
- Q3: Should per-value style rules be configured through the properties panel,
  the data-binding editor, or a dedicated DataGrid column editor?
- Q4: Should `uiGrid` be a hard requirement for implementation, or may `/plan`
  choose between a wrapped `uiGrid` adapter and improving the native renderer
  after evaluating technical fit?
