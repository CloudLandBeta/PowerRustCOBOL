# Tasks — The Form Designer magnifier

- **Spec:** ./spec.md  **Plan:** ./plan.md  **Date:** 2026-10-06

- [x] **T1 — `magnifier.rs`** (R3, R5, R6; AC3, AC4)
  - `MagnifierFeed`, `capture(ctx, layer, start, pointer, form_rect)`, `paint(ui, view_rect, feed)`, text re-laid at 4×.
  - Verify: unit tests (scale about the pointer, 36 px text, clipped to the form, empty feed).

- [x] **T2 — Canvas capture and the toolbar toggle** (R1, R7, R8; AC1, AC5)
  - `magnifier_on` (off on open), `magnifier_feed`; capture at the end of the canvas pass only while on; `ToggleMagnifier` button and dispatch.
  - Verify: the toggle is not an edit; the canvas paints the same with it off.

- [x] **T3 — The Zoom section in the toolbox** (R2, R4, R9; AC2, AC4)
  - First section, above Common; the hint when the pointer is off the form; the scroll budget shrinks by the section's height.
  - Verify: toolbox tests for order, absence when off, and the height budget.

- [x] **T4 — i18n, Guide, changelog, version**
  - `tb_magnifier`, `cat_zoom`, `magnifier_hint` ×6; "The magnifier" in the Guide; delete its five translations; CHANGELOG; bump `z`.

- [x] **T5 — Finalize** (AC6)
  - `cargo test -p cobolt-forms --features render`; `cargo test -p cobolt-ide --bin cobolt-ide --tests`; spec status → implemented.
