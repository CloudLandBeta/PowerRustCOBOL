# Plan — Universal border radius + rounded content clipping

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-21

## 1. Approach

The shared renderer `cobolt_forms::paint::draw_control` already computes a single
`corner` radius (paint.rs:965) and draws every control's **fill and border** with
it, for designer/preview/run/compiled alike. The work is therefore concentrated:
generalise `corner` to a universal property, round the few content draws that can
overflow (images, charts), surface the property in the pane, and seed defaults.

- **Universal radius (R1, R5).** Replace the `corner` match with a single helper
  `corner_radius(ctrl)` that reads the canonical **`CornerRadius`**, falls back to
  the legacy container key **`BorderRadius`** (alias) and then to a per-type
  default, and **clamps** to `0.5 * min(w, h)` (R4). Because the fill/border
  already use `corner`, R2 is satisfied once this returns the right value. Radius
  `0` ⇒ square + no rounding, unchanged (R3/R7).
- **Image content (R3).** PictureBox image is drawn with `painter.image()`
  (paint.rs:1126) and, in the IDE, `draw_picturebox` (app.rs:5637). Swap both to a
  rounded **textured `epaint::RectShape`** (`rounding` + `fill_texture_id` + `uv`)
  using the control's radius — native, correct over any background (spec §1, Q3).
- **Chart content (R3).** `draw_chart_preview` hardcodes an `8.0` corner for the
  glass/solid frame; make it read the control's radius (fallback 8 to preserve the
  current look). Chart marks are already drawn through a `rect.shrink(1.0)`
  axis-aligned clip; that stays (marks are inset — corner overflow is negligible).
- **Other controls (R3).** Text/value controls inset their content, so the
  rounded fill/border is the visible result and no extra clipping is needed.
  Container child-clipping keeps the existing spec-012 **axis-aligned** `clip_rect`
  (the rounded corners are cosmetic on the frame); true rounded child masking is a
  documented non-goal for v1 (Q3 residual), since children rarely reach corners.
- **Property pane (R1).** Add **one** "Border radius" row for every bordered
  control. Today only the container sections show it; add a shared row in the
  per-type sections (or a small helper invoked from each bordered arm).
- **Defaults (R7, Q2).** Seed `BorderRadius` on every bordered control with its
  **current effective** value so existing forms don't shift: Button `3`, charts
  `8`, everything else `0`.

## 2. Affected crates / files
- `crates/cobolt-forms/src/paint.rs` — `border_radius(ctrl)` helper (read +
  alias + clamp); use it for `corner`; pass it into `draw_chart_preview`; rounded
  textured `RectShape` for the PictureBox image; unit tests for the helper.
- `crates/cobolt-forms/src/model.rs` — add `BorderRadius` defaults to all bordered
  controls (Button/TextBox/ComboBox/ListBox/ListView/PictureBox/DataGrid/
  NumericUpDown/DateTimePicker/ProgressBar/Slider/Shape + charts; containers
  already have it); extend a model test.
- `crates/cobolt-ide/src/app.rs` — `draw_picturebox` rounded `RectShape`; (run/
  preview already delegate faces to `draw_control`, so most controls inherit the
  radius automatically — audit the few inline draws).
- `crates/cobolt-ide/src/panels/properties.rs` — universal "Border radius" row for
  bordered controls (reuse `int_row_inline`).
- `docs/developers-guide-en.md` — document the universal property + the clipping
  rule and its residual limitation (text/scroll layer, container children).
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — patch bump (fix).

## 3. Data / model changes
- **New prop:** `BorderRadius` (Int) on every bordered control (defaults per §1).
  Serialises generically in `.cfrm`; old files load unchanged (missing ⇒ default).
- **Back-compat:** the container key `BorderRadius` (spec 012) is still **read** as
  an alias by `corner_radius()` and remains in old files; new controls (and
  containers) write `CornerRadius`. No XML migration required (both keys coexist;
  reader prefers `CornerRadius`). Container model defaults switch
  `BorderRadius` → `CornerRadius`; spec-012 tests asserting `BorderRadius` update
  accordingly.
- **No AST/runtime/codegen change.**

## 4. Key decisions & alternatives
- **One name `CornerRadius`, `BorderRadius` read-only alias (Q1, operator's
  choice)** — Why: one pane row, one model key; Button already uses
  `CornerRadius`. Containers migrate to `CornerRadius`, reading the legacy
  `BorderRadius` for old files. Rejected: keep both names per-control (duplicated
  UI, drift).
- **Radius via geometry, not a scissor (Q3)** — Why: egui has no rounded/shape
  clip; `RectShape.rounding` + textured `RectShape` give correct rounded fill and
  images over any background on the pinned 0.29 (no upgrade). Rejected:
  corner-mask against the resolved background (breaks over a background image) —
  demoted to an unused fallback.
- **Preserve current effective defaults (Q2)** — Why: AC6 (existing forms
  unchanged). Rejected: default 0 everywhere (would un-round Buttons/charts).
- **Container children stay axis-aligned clipped (Q3 residual)** — Why: rounded
  child masking needs per-pixel masking egui can't do; children rarely reach the
  corner. Documented limitation.

## 5. Risks & mitigations
- **Radius too large → degenerate shape** → clamp to `0.5 * min(w,h)` (R4) in the
  helper; unit-tested.
- **PictureBox `RectShape` regressions** (tint/`ShowFrame`/SizeMode) → keep the
  existing tint + frameless behaviour; only the draw call changes; verify the
  frameless (no-card) path still shows the rounded image.
- **Charts double-rounding / clip mismatch** → only the frame corner becomes
  dynamic; the `shrink(1.0)` content clip is unchanged.
- **Property-pane clutter** → a single compact row; for containers it replaces the
  existing "Border radius" row (no duplicate).
- **i18n** → one new `Tr` label; existing parity test guards the six languages.

## 6. Test strategy
- **`cobolt-forms` (paint, `--features render`)** — unit-test `border_radius`:
  reads `BorderRadius`; falls back to `CornerRadius` then type default; clamps to
  half the min dimension; `0` stays `0`. Reports each asserted value.
- **`cobolt-forms` (model)** — assert every bordered control exposes
  `BorderRadius` with the documented default; non-bordered/non-visual controls do
  not; a control with a set radius round-trips through `.cfrm` (xml test).
- **`cobolt-ide` (i18n)** — parity test stays green with the new label.
- **Manual/visual** — in the IDE: set radius on a TextBox, PictureBox (with an
  image), chart, and container; confirm rounded fill/border on the canvas, a
  rounded **image** (no corner spill) over both a solid form bg and a background
  image, and identical look in preview + running form; set radius 0 and confirm
  the control is square/unclipped as before.

## 7. Steering compliance
- [x] i18n: the new "Border radius" label added as `Tr` in 6 languages.
- [x] Generated-code banner + regenerate-on-action unchanged (render-only).
- [x] English dev guide updated (translations untouched).
- [x] Fix vs feature: **fix** per the standing pre-production directive → patch
  bump + CHANGELOG.
- [x] No "cobolt" in user-facing text; COBOL identifiers/source English.
