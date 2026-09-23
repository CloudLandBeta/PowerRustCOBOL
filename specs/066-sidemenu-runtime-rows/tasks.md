# Tasks — SideMenu: run-time rows and hosted controls

- **Plan:** ./plan.md   **Date:** 2026-09-22
- Derived from the plan at the operator's `/implement` (no separate `/tasks`
  review). Phases ship in order; each ends green.

## Phase A — run-time rows

- [x] **T1** `menu.rs`: `RuntimeRow`, `MAX_DEPTH` pub, `merge_rows`, and the
      `runtime` operations (add/replace, section, set label/icon/badge/enabled/
      action, remove with descendants, clear, count, has) with designed-id and
      depth refusals. *Verify:* `cargo test -p cobolt-forms --lib menu` — a
      table of cases, reported.
- [x] **T2** `model.rs`: `RuntimeRows` in `runtime_property_names_for("SideMenu")`.
      *Verify:* `cargo test -p cobolt-forms --lib runtime`.
- [x] **T3** `render.rs` SideMenu arm merges `RuntimeRows`. *Verify:* engine
      test — a run-time row is laid out after the designed rows and a click sets
      `SelectedItemId` + fires `onMenuItemClick`.
- [x] **T4** interpreter: `set_designed_menu`; SideMenu branches of
      `ADDITEM`/`REMOVEITEM`/`CLEAR`/`GETCOUNT`; new `ADDSECTION`,
      `SETITEMLABEL`/`ICON`/`BADGE`/`ENABLED`/`ACTION`, `HASITEM`;
      `is_known_method`. *Verify:* runtime test with a COBOL program (AC1, AC4),
      `every_dispatched_control_method_is_spellable_inline`.
- [x] **T5** seed designed menus at the three hosts: `form_gui.rs`, compiled
      template, host child seed. *Verify:* build + source assertions.
- [x] **T6** host `control_prop`; shell merges live rows + live
      `SelectedItemId`. *Verify:* shell test feeding `RuntimeRows` (AC2, AC6b).
- [x] **T7** parity test: engine and shell lay out the same merged rows (AC7).
- [ ] **T8** editor autocomplete, KB tables + `chunked.data`, Guide section.

## Phase B — header band

- [ ] **T9** model: `HeaderBandHeight`, `IsSideMenuHeader`,
      `sync_side_menu_header_panels`, `side_menu_header_subtree`, backfill.
- [ ] **T10** sidebar: `RowKind::HeaderBand`, menu band moves while expanded,
      0 when collapsed.
- [ ] **T11** render: skip a collapsed menu's header subtree; designer sync +
      drag-lock; properties row.
- [ ] **T12** shell/host: `draw_side_menu_band` generalises the footer pass.
- [ ] **T13** tests (AC6), KB, Guide.

## Phase C — rows that host a control

- [ ] **T14** `MenuItem.control` (skip when none; hash unchanged).
- [ ] **T15** layout rect for hosted rows; `side_menu_row_rect` in
      `resolved_rect`; shell band pass per hosted row; hidden when collapsed.
- [ ] **T16** `SetItemControl`; menu editor *Hosted control* (i18n ×6).
- [ ] **T17** tests (AC6a), KB, Guide.

## Close

- [ ] **T18** full sweeps; CHANGELOG + `z` bump; acceptance criteria checked.
