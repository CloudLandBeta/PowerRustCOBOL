# Tasks — Universal border radius + rounded content clipping

- **Status:** done (2026-06-21)
- **Plan:** ./plan.md   **Date:** 2026-06-21

Ordered, small, independently-verifiable. The project stays green after each task.

- [x] **T1 — `corner_radius(ctrl)` helper + universal `corner`** (R1,R2,R4,R5)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: add `pub fn corner_radius(ctrl: &Control) -> f32` — read canonical
    `CornerRadius`, else legacy `BorderRadius` (container alias), else a per-type
    default (Button 3, charts 8, else 0); clamp to `0.5 * min(w,h)` and `>= 0`.
    Replace the `corner` match (paint.rs:965) with a call to it. Add unit tests
    (read / alias / default / clamp / zero-stays-zero).
  - Verify: `cargo test -p cobolt-forms --features render corner_radius` green;
    fill/border of a control with `CornerRadius` set now rounds.

- [x] **T2 — Rounded PictureBox image (shared renderer)** (R3)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: replace the PictureBox `painter.image(...)` (≈paint.rs:1126) with a
    textured `epaint::RectShape` (`rounding` from `border_radius`,
    `fill_texture_id`, `uv`, tint) so the image is clipped to the rounded frame;
    keep the existing tint / `ShowFrame` (frameless) / SizeMode behaviour.
  - Verify: `cargo build -p cobolt-forms --features render`; manual: a PictureBox
    with radius > 0 shows a rounded image with no corner spill.

- [x] **T3 — Charts honour the radius** (R3)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: in `draw_chart_preview`, replace the hardcoded `8.0` frame corner with the
    control's `border_radius` (default 8 to preserve the current look); content
    clip stays `rect.shrink(1.0)`.
  - Verify: `cargo test -p cobolt-forms --features render` green; manual: a chart
    with radius 0 is square, with radius > 0 the frame rounds.

- [x] **T4 — `CornerRadius` defaults on bordered controls + model test** (R1,R7)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: add `CornerRadius` (Int) to every bordered control's defaults with its
    current effective value (Button 3, charts 8, TextBox/ComboBox/ListBox/
    ListView/PictureBox/DataGrid/NumericUpDown/DateTimePicker/ProgressBar/Slider/
    Shape 0). Migrate containers' `BorderRadius` default → `CornerRadius` (0).
    Update the spec-012/015 container tests that assert `BorderRadius` to
    `CornerRadius`. Extend a model test asserting the defaults exist (and that
    non-bordered/non-visual controls do not get it).
  - Verify: `cargo test -p cobolt-forms` green.

- [x] **T5 — `.cfrm` round-trip + back-compat read** (R5)
  - Files: `crates/cobolt-forms/src/xml.rs` (test only)
  - Do: round-trip a control with a non-default `CornerRadius`; assert a legacy
    file carrying only the container `BorderRadius` still rounds via the alias
    (`corner_radius` reads it).
  - Verify: `cargo test -p cobolt-forms` green.

- [x] **T6 — Rounded PictureBox in the IDE run/preview image path** (R3,R6)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: update `draw_picturebox` (app.rs:5637) to a rounded textured `RectShape`
    using the radius; audit the other inline image draws so run/preview match the
    designer (most faces already delegate to `draw_control`).
  - Verify: `cargo build -p cobolt-ide`; manual: rounded image identical in
    designer, preview, and running form.

- [x] **T7 — Universal "Border radius" property row** (R1,R8)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `crates/cobolt-ide/src/i18n.rs`
  - Do: show a single "Border radius" row (reuse `int_row_inline`, range `0..=400`)
    for every bordered control; replace the existing container border-radius rows
    so there is no duplicate. Add the label as a `Tr` key in all six languages.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n` green;
    the row appears for TextBox, PictureBox, chart, Button, container.

- [x] **T8 — Docs (English guide)** (R8)
  - Files: `docs/developers-guide-en.md`
  - Do: document the universal **Border radius** property, the rounded-fill/border
    + rounded-image behaviour, `0 = square/no clipping`, and the residual
    limitations (editable text/scroll layer, container children stay
    axis-aligned). English only; translations untouched.
  - Verify: section present; no translation files modified.

- [x] **T9 — Finalize** (all ACs)
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: patch bump (fix, per standing directive) + CHANGELOG entry.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-forms --features
    render` + `cargo test -p cobolt-ide i18n` green; manual AC walkthrough
    (AC1–AC7) per plan §6.

## Done criteria
Every acceptance criterion (AC1–AC7) is covered: AC1 (T4,T7), AC2 (T1), AC3
(T2,T3,T6), AC4 (T1), AC5 (T6), AC6 (T4,T5), AC7 (T7,T8). Tests pass, docs
updated, change committed as a fix. Do **not** commit/push unless the operator
asks.
