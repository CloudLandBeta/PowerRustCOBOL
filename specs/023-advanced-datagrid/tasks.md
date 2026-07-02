# Tasks — Advanced DataGrid

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-06-30

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

---

## Layer 1: Feasibility and model compatibility

- [x] **T1 — Evaluate `uiGrid` integration fit** (R26)
  - Files: `specs/023-advanced-datagrid/plan.md` if findings require a plan
    correction.
  - Do: Verify `uiGrid` release `rust-v1.0.6` license, package shape, Rust
    version compatibility, egui compatibility, available features, and whether
    it can support static designer rendering plus interactive runtime rendering
    behind PowerRustCOBOL's existing `DataGrid` model.
  - Verify: Record findings in the task result. If accepted, implementation
    remains adapter-backed; if rejected, continue with native renderer. Run
    `cargo build -p cobolt-forms` after any dependency experiment. Covers AC16.
  - Result: Skipped by user decision for this iteration. Continue with the
    native renderer implementation path and do not add a `uiGrid` dependency,
    compatibility shim, or adapter experiment under spec 023.

- [x] **T2 — Add advanced DataGrid model defaults** (R5, R6, R7, R9, R11, R15,
  R16, R18, R22, R24)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: Add typed advanced DataGrid metadata/defaults for column width/order,
    row height overrides, filters, style rules, framed cells, gauges, freeze
    panes, CSV export mode, and grid-line style while preserving `Columns` and
    `Rows`.
  - Verify: `cargo test -p cobolt-forms datagrid_advanced_model --features render`
    passes. Tests assert defaults, property names, and legacy fallback from
    `Columns`. Covers AC4, AC5, AC6, AC8, AC10, AC11, AC12, AC15, AC16.

- [x] **T3 — Persist advanced DataGrid metadata** (R24)
  - Files: `crates/cobolt-forms/src/xml.rs`,
    `crates/cobolt-forms/src/model.rs`
  - Do: Round-trip optional advanced metadata deterministically; load old forms
    without advanced metadata; preserve legacy `Columns`, `Rows`, bindings,
    events, and layout.
  - Verify: `cargo test -p cobolt-forms datagrid_advanced_cfrm --features render`
    passes. Tests include simple legacy grids and advanced grids with style,
    freeze, filter, row, and column metadata. Covers AC4, AC5, AC6, AC8, AC11,
    AC15, AC16, AC17.

## Layer 2: Shared grid engine and renderer

- [x] **T4 — Extract DataGrid layout engine** (R1, R2, R3, R4, R17, R25)
  - Files: `crates/cobolt-forms/src/render.rs`,
    `crates/cobolt-forms/src/datagrid.rs` or equivalent module
  - Do: Centralize DataGrid parsing/layout into a helper that returns viewport
    rectangles, visible row range, visible/frozen columns, header/body regions,
    and scroll bounds.
  - Verify: `cargo test -p cobolt-forms datagrid_layout --features render`
    passes. Tests assert 100,000-row virtual range, clipped body bounds, fixed
    header, horizontal scroll math, and frozen alignment. Covers AC1, AC2, AC3,
    AC11.

- [x] **T5 — Implement virtual scroll and frozen panes** (R1, R2, R3, R4, R16,
  R17, R25)
  - Files: `crates/cobolt-forms/src/render.rs`,
    DataGrid layout module
  - Do: Render only visible rows/columns plus bounded buffer; support fast
    vertical/horizontal scroll; keep headers and configured frozen rows/columns
    visible and aligned.
  - Verify: `cargo test -p cobolt-forms datagrid_virtual_scroll --features render`
    passes and `cargo build -p cobolt-ide` succeeds. Manual check: 100,000-row
    grid scrolls without drawing outside bounds. Covers AC1, AC2, AC3, AC11.

- [x] **T6 — Add column and row resizing interactions** (R5, R6)
  - Files: `crates/cobolt-forms/src/render.rs`,
    DataGrid layout module,
    `crates/cobolt-forms/src/model.rs`
  - Do: Add hit testing and drag behavior for column edges and row edges; update
    column widths and row height/global or per-row overrides as property
    updates.
  - Verify: `cargo test -p cobolt-forms datagrid_resize --features render`
    passes. Manual check: width/height changes persist after save/reload,
    preview, and run. Covers AC4, AC5.

- [x] **T7 — Add filtering and column reorder controls** (R7, R8, R9, R10)
  - Files: `crates/cobolt-forms/src/render.rs`,
    DataGrid layout module,
    `crates/cobolt-forms/src/model.rs`
  - Do: Add header reorder icons, filter input rows, AND-chained filters, stable
    column ids, and metadata-preserving reorder behavior.
  - Verify: `cargo test -p cobolt-forms datagrid_filter_reorder --features render`
    passes. Tests assert two active filters chain correctly and column reorder
    preserves binding/style/filter/width/sort metadata. Covers AC6, AC7.

- [x] **T8 — Render rich cells, gauges, fonts, and grid-line styles** (R11, R12,
  R13, R14, R15, R22, R23, R25)
  - Files: `crates/cobolt-forms/src/render.rs`,
    DataGrid layout module,
    `crates/cobolt-forms/src/paint.rs` if shared helpers are needed
  - Do: Render per-column foreground/background, framed rounded pills,
    value-style rules, numeric gauges, grid fonts, and line styles `Solid`,
    `Dash`, `Dots`, and `None`.
  - Verify: `cargo test -p cobolt-forms datagrid_rich_cells --features render`
    passes. Manual visual check: status values render as colored pills and
    grid-line styles differ in Designer, Preview, and Run Form. Covers AC8, AC9,
    AC10, AC15.

- [x] **T9 — Add selectable/copyable cell text** (R20, R21)
  - Files: `crates/cobolt-forms/src/render.rs`,
    DataGrid layout module
  - Do: Support text selection inside cells and fallback copy of selected cell,
    row, or range according to `SelectionMode` when no partial text is active.
  - Verify: `cargo test -p cobolt-forms datagrid_selection_copy --features render`
    passes. Manual check: select text inside a cell and copy it; copy row/cell
    selection with keyboard shortcut. Covers AC13, AC14.

## Layer 3: IDE surfaces, methods, and data binding

- [x] **T10 — Add DataGrid Column Editor and properties** (R5, R6, R7, R9, R11,
  R12, R13, R15, R16, R18, R22)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`,
    `crates/cobolt-ide/src/panels/data_grid_columns.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: Add a dedicated Column Editor for width/order, filters, style rules,
    framed cells, gauges, frozen rows/columns, CSV export mode, and grid-line
    style; keep Properties panel compact with summaries and editor button.
  - Verify: `cargo test -p cobolt-ide datagrid_column_editor` and
    `cargo build -p cobolt-ide` pass. Manual check: changes survive save,
    reload, preview, and run. Covers AC4, AC5, AC6, AC8, AC10, AC11, AC12,
    AC15.

- [x] **T11 — Add runtime methods and autocomplete** (R16, R18, R19, R20, R21,
  R27)
  - Files: `crates/cobolt-runtime/src/interpreter.rs`,
    `crates/cobolt-ide/src/panels/editor.rs`
  - Do: Expose methods/properties for setting filters, clearing filters,
    freezing rows/columns, exporting CSV, reading selection/copy text, resizing,
    and refreshing bound data.
  - Verify: `cargo test -p cobolt-runtime datagrid_methods` and
    `cargo test -p cobolt-ide methods_for_type_universal_and_specific_coverage`
    pass. Covers AC12, AC13, AC14.

- [x] **T12 — Preserve data binding through advanced metadata** (R24, R28)
  - Files: `crates/cobolt-ide/src/app.rs`,
    `crates/cobolt-ide/src/data_binding_guardian.rs`,
    `crates/cobolt-codegen/src/data_binding.rs`
  - Do: Ensure reordering/styling/filtering/freezing columns never changes
    binding field identity; keep `RefreshBinding()` and generated binding rows
    working after advanced metadata is added.
  - Verify: `cargo test -p cobolt-ide datagrid_binding_metadata`,
    `cargo test -p cobolt-runtime datagrid_refresh_binding_updates_rows_from_cobol_table`,
    and `cargo test -p cobolt-codegen data_binding` pass. Covers AC16, AC17.

- [x] **T13 — Update CSV export behavior** (R18, R19)
  - Files: `crates/cobolt-codegen/src/lib.rs`,
    `crates/cobolt-runtime/src/interpreter.rs`,
    DataGrid renderer/model files
  - Do: Add visible export command behavior and `CSVExportMode` support for
    filtered vs all rows using displayed column order and delimiter.
  - Verify: `cargo test -p cobolt-codegen datagrid_csv_export` and
    `cargo test -p cobolt-runtime datagrid_csv_export` pass. Manual check:
    export button appears only when enabled and CSV output matches displayed
    order. Covers AC12.

- [x] **T14 — Add i18n for advanced DataGrid UI** (R7, R9, R18)
  - Files: `crates/cobolt-ide/src/i18n.rs`,
    DataGrid properties/editor call sites
  - Do: Add every new label, tooltip, command, validation message, and menu item
    as `Tr` fields in EN/ES/PT/JA/ZH/FR; remove hard-coded user-facing strings.
  - Verify: `cargo test -p cobolt-ide i18n` passes with no empty advanced
    DataGrid translations. Covers AC18.

## Layer 4: Docs and final verification

- [x] **T15 — Document Advanced DataGrid** (R1-R23)
  - Files: `docs/developers-guide-en.md`
  - Do: Document virtual scrolling, resizing, reorder/filter controls, rich cell
    formatting, gauges, font behavior, frozen panes, CSV export, selectable
    text, grid-line styles, methods, and data-binding compatibility. Do not edit
    translated guides.
  - Verify: Review rendered Markdown in the IDE documentation viewer or preview.
    Confirm `docs/developers-guide-es.md`, `-pt`, `-jp`, `-cn`, and `-fr` are
    untouched. Covers AC19.

- [x] **T16 — Finalize version, changelog, and full verification**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: Apply required version bump and top changelog entry dated 2026-06-30,
    run formatting, touched-crate tests, and final build/manual checks. Do not
    commit or push.
  - Verify: `cargo fmt`, `cargo test -p cobolt-forms --features render`,
    `cargo test -p cobolt-ide`, `cargo test -p cobolt-runtime`,
    `cargo test -p cobolt-codegen`, and `cargo build -p cobolt-ide` pass or
    failures are reported exactly. Manual launch: `cargo run -p cobolt-ide`;
    verify Designer/Preview/Run parity for the advanced DataGrid demo. Covers
    all ACs.
  - Result: Version bumped to `1.27.50` and `CHANGELOG.md` updated for
    2026-06-30. Targeted touched-crate checks passed. `cargo build -p
    cobolt-ide` reached the linker and failed with `ld: write() failed,
    errno=28`; `df -h . target` showed the mounted volume at 100% capacity with
    172 MiB available. Manual launch was not run because the IDE binary could
    not be linked under the current disk-space condition.

---

## AC ↔ Task mapping

| Acceptance criterion | Task coverage |
| --- | --- |
| AC1 — 100,000-row virtual rendering | T4, T5 |
| AC2 — clipped rows inside grid bounds | T4, T5 |
| AC3 — fixed header and frozen horizontal behavior | T4, T5 |
| AC4 — column resize persists | T2, T3, T6, T10 |
| AC5 — row resize persists | T2, T3, T6, T10 |
| AC6 — reorder preserves metadata | T2, T3, T7, T10 |
| AC7 — chained filters | T7 |
| AC8 — status pills/value style rules | T2, T3, T8, T10 |
| AC9 — font settings in runtime | T8 |
| AC10 — numeric gauges with text | T2, T8, T10 |
| AC11 — frozen rows/columns aligned | T4, T5, T10 |
| AC12 — CSV export command/order | T11, T13 |
| AC13 — selectable/copyable cell text | T9, T11 |
| AC14 — copy fallback by selection mode | T9, T11 |
| AC15 — grid-line styles | T2, T3, T8, T10 |
| AC16 — legacy forms remain compatible | T1, T2, T3, T12 |
| AC17 — data-bound grids still populate | T12 |
| AC18 — i18n coverage | T14 |
| AC19 — English guide docs | T15 |

## Done criteria

All acceptance criteria in `spec.md` are satisfied, every task verification has
been run and reported with real results, docs and i18n are updated, the required
version/changelog changes are present, and no commit or push is made unless the
operator explicitly asks.
