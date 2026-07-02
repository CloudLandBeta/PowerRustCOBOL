# Plan — Advanced DataGrid

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-30

## 1. Approach

Evolve the existing native `DataGrid` instead of replacing its public model. The
current grid is rendered in the shared `cobolt-forms` engine from `Columns` and
`Rows`, with properties edited in the IDE and runtime methods exposed through
the interpreter. The advanced grid will keep that compatibility layer, add a
typed column/layout/style model beside the legacy string properties, and render
through the unified renderer so Designer, Preview, Run Form, and compiled output
stay visually consistent (R1-R4, R24-R25).

Implement the feature in layers. First add model/XML compatibility for advanced
column metadata, scroll/freeze/filter state, row sizing, selectable text,
grid-line style, cell formatting, and export behavior. Then refactor the
renderer into a reusable DataGrid engine that computes visible virtual rows,
visible/frozen columns, cell rectangles, filters, gauges, framed pills, and line
styles before drawing. Finally add IDE property/editor surfaces, runtime
methods, codegen/autocomplete integration, docs, and i18n (R5-R23, R27-R28).

`uiGrid` is not mandatory for implementation. It remains a candidate only if an
early evaluation proves it is MIT-licensed, compatible with the workspace Rust
and egui versions, supports virtual scrolling/frozen panes/selection well, and
can be wrapped behind PowerRustCOBOL's existing `.cfrm` model without exposing
third-party API details. If it fails any of those gates, continue with the
native renderer plan (R26).

T1 implementation finding: direct `uiGrid` adoption is skipped by user decision
for this feature iteration. Implementation proceeds with the native renderer
path and no `uiGrid` dependency, compatibility shim, or adapter experiment will
be added under spec 023.

Defaults chosen from the spec's open questions:

- CSV export defaults to the currently displayed rows after filters, with an
  `CSVExportMode` property allowing `Filtered` or `AllRows`.
- Row resizing supports both a global `RowHeight` and optional per-row overrides.
- Value-to-style rules live in a dedicated DataGrid Column Editor, with compact
  read-only summaries in the Properties panel.
- `uiGrid` is optional, not a hard requirement.

## 2. Affected crates / files

- `crates/cobolt-forms/src/model.rs` — add advanced DataGrid column metadata,
  grid layout/style/filter/freeze/export properties, helper parsers, and
  backward-compatible defaults.
- `crates/cobolt-forms/src/xml.rs` — round-trip optional advanced DataGrid
  metadata while keeping legacy `Columns`/`Rows` forms valid.
- `crates/cobolt-forms/src/render.rs` and/or a new
  `crates/cobolt-forms/src/datagrid.rs` — implement virtual scrolling, clipped
  viewports, frozen rows/columns, resizing interactions, filters, selection,
  framed cell rendering, numeric gauges, and grid-line styles.
- `crates/cobolt-ide/src/panels/properties.rs` — expose basic advanced
  properties and entry point for the Column Editor.
- `crates/cobolt-ide/src/panels/data_grid_columns.rs` (new) — edit column order,
  widths, styles, value rules, gauges, filter settings, freeze options, and CSV
  behavior.
- `crates/cobolt-ide/src/i18n.rs` — add all new labels, commands, tooltips, and
  validation messages in EN/ES/PT/JA/ZH/FR.
- `crates/cobolt-ide/src/panels/editor.rs` — add autocomplete for new DataGrid
  methods/properties.
- `crates/cobolt-runtime/src/interpreter.rs` — support new DataGrid
  properties/methods such as filter, freeze, export, selection, and row/column
  sizing.
- `crates/cobolt-codegen/src/lib.rs` — keep generated CSV/export helpers and
  DataGrid initialization deterministic and parseable.
- `docs/developers-guide-en.md` — document the advanced DataGrid workflow and
  properties.
- `CHANGELOG.md` and `crates/cobolt-ide/src/version.rs` — feature/fix entry and
  version bump during implementation.

## 3. Data / model changes

Add optional advanced DataGrid metadata while preserving existing fields:

```text
DataGridAdvanced
  schema_version: u16
  columns: Vec<DataGridColumn>
  frozen_columns: usize
  frozen_rows: usize
  row_height: u16
  row_overrides: Vec<RowHeightOverride>
  filters: Vec<DataGridFilter>
  csv_export_mode: Filtered | AllRows
  grid_line_style: Solid | Dash | Dots | None

DataGridColumn
  id: String
  title: String
  source_name: String
  value_type: String
  width: f32
  visible: bool
  frozen: bool
  alignment: Left | Center | Right
  foreground: Option<String>
  background: Option<String>
  frame: Option<DataGridCellFrame>
  value_rules: Vec<DataGridValueStyleRule>
  gauge: Option<DataGridGauge>
  filter_enabled: bool
```

Legacy `Columns` remains the compatibility source for simple forms and initial
migration. `Rows` remains the runtime/data-binding row transport. Advanced
metadata, when present, controls display order, width, styles, filters, gauges,
and freezing. Missing advanced metadata is derived from `Columns` at load/render
time without rewriting the form unless the user edits advanced grid settings.

`.cfrm` gains optional DataGrid child metadata or a deterministic encoded
property block for advanced settings. The implementation plan should prefer the
repo's existing XML patterns and must prove old forms round-trip unchanged
except for intentionally seeded missing default properties.

## 4. Key decisions & alternatives

- Decision: keep the native `DataGrid` public model and add advanced metadata.
  Why: data binding, codegen, runtime methods, `.cfrm`, and designer selection
  already depend on the existing control. Rejected: direct wholesale replacement
  with third-party API types.
- Decision: reject direct `uiGrid` adoption for this iteration and continue with
  the native renderer. Why: license is acceptable, but its Rust/egui adapter
  targets newer Rust and egui versions than the workspace permits. Rejected:
  adding a dependency that would violate Rust 1.75 and egui/eframe 0.29
  compatibility.
- Decision: use a dedicated Column Editor for rich settings. Why: per-column
  style rules, gauges, filters, and frozen state are too dense for the normal
  Properties panel. Rejected: stuffing all advanced settings into the standard
  two-column property grid.
- Decision: CSV exports filtered rows by default. Why: users expect the visible
  working set to be exported. Rejected: all-rows-only export, because it makes
  filters less useful.
- Decision: both global and per-row heights are supported. Why: the grid needs a
  sane default plus targeted overrides for rich rows. Rejected: per-row-only
  resizing due to model bloat for normal grids.

## 5. Risks & mitigations

- Risk: large grids can become slow if all rows are measured/drawn. Mitigation:
  compute filtered row indexes once per data/filter signature and draw only the
  visible range plus a small buffer.
- Risk: frozen rows/columns can drift out of alignment. Mitigation: centralize
  grid layout math in a single engine and test rectangle calculations.
- Risk: advanced metadata can break old `.cfrm` files. Mitigation: keep
  `Columns`/`Rows` compatibility and add round-trip tests for legacy forms.
- Risk: selecting text inside an immediate-mode grid can conflict with row
  selection and scroll. Mitigation: use explicit selection modes and make text
  selection take precedence only while a cell text interaction is active.
- Risk: value-style rules can accidentally break data binding. Mitigation:
  store style rules by stable column id/source field and add guardian/regression
  tests that reordering/styling does not alter binding mappings.
- Risk: `uiGrid` may not support static designer rendering or the workspace egui
  version. Mitigation: keep it behind a one-task spike and proceed natively if
  it fails.

## 6. Test strategy

- `cobolt-forms` tests:
  - advanced DataGrid metadata defaults and legacy `Columns` migration;
  - `.cfrm` round-trip for legacy and advanced grids;
  - virtual row/column layout calculations for 100,000 rows;
  - filter chaining and displayed-row indexes;
  - frozen row/column alignment;
  - grid-line style, framed pill, gauge, and text clipping behavior.
- `cobolt-ide` tests:
  - Properties panel shows advanced entry points and localized strings;
  - Column Editor mutates column width/order/style/filter/gauge/freeze metadata;
  - i18n has no empty advanced DataGrid labels in all six languages;
  - editor autocomplete includes new DataGrid methods/properties.
- `cobolt-runtime` tests:
  - DataGrid methods update filters, freeze panes, selection, export mode, and
    sizing properties;
  - data-bound `RefreshBinding()` still populates rows after columns are styled
    or reordered.
- `cobolt-codegen` tests:
  - generated CSV/export helper remains deterministic and parseable;
  - generated/run regeneration preserves existing banner contract.
- Manual verification:
  - launch IDE, design an advanced grid with status pills, gauges, filters,
    frozen panes, resized rows/columns, and CSV export;
  - run the form and verify Designer/Preview/Run visual parity, scroll speed,
    copy behavior, and exported CSV.

## 7. Steering compliance

- [ ] i18n: all new UI strings in six languages.
- [ ] Generated-code banner + regenerate-on-action contract preserved.
- [ ] English developer guide updated; translations untouched.
- [ ] Feature/fix versioning handled during implementation per `CONVENTIONS.md`.
- [ ] No "cobolt" in user-facing text; COBOL identifiers remain English.
