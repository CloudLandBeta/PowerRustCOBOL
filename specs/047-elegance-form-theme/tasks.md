# Tasks — Elegance form theme

- **Status:** in progress → code complete (manual/visual checks pending)
- **Plan:** ./plan.md   **Date:** 2026-08-08

Ordered, small, independently-verifiable tasks. The tree stays **green after
every task**, and Liquid Glass / asset-pack forms stay unchanged throughout
(R10). Tasks T1–T5 produce **no visual change at all** — they build the
mechanism and the regression guard first, so that every later painting task has
something to fail against.

Sequencing note: selection wiring (T5) lands *before* any Elegance painting, so
from T5 onward you can pick Elegance in the IDE and watch each subsequent task
convert another slice of the form. Until T7 it will correctly still look like
Liquid Glass.

---

## Phase 1 — Mechanism + regression guard (no visual change)

- [x] **T1 — Catalog entry** (R1, R2)
  - Files: `crates/cobolt-forms/src/theme.rs`
  - Do: add `pub const ELEGANCE: &str = "elegance"` and
    `FormTheme::elegance()` (`ThemeKind::Procedural`, display name
    `"Elegance"`); `ThemeCatalog::builtin()` returns Liquid Glass **then**
    Elegance. Do **not** touch `resolve_theme_id`. Keep this module free of
    `egui`/`elegance` types — it is not behind the `render` feature.
  - Verify: `cargo test -p cobolt-forms elegance_catalog` green — new test
    asserts `builtin().ids() == ["liquid-glass", "elegance"]`, both
    `Procedural`, display names exact; existing
    `resolution_precedence_form_then_project_then_glass` and
    `resolve_unknown_id_falls_back_to_glass` still pass unchanged. Test
    **reports** the catalog ids in order.

- [x] **T2 — `SurfaceStyle` on the wire** (R12 mechanism)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: add `pub enum SurfaceStyle { LiquidGlass, Elegance }` (default
    `LiquidGlass`), `pub fn set_surface_style(ctx, SurfaceStyle)` and
    `active_surface_style(ctx) -> SurfaceStyle`, mirroring `set_glass_style` /
    `active_glass_style`. Nothing reads it yet.
  - **Named `SurfaceStyle`, not `FormStyle`:** a per-control *property* called
    `"FormStyle"` (a bool) already exists and means something else entirely —
    two different concepts under one name would be a standing trap.
  - Verify: `cargo test -p cobolt-forms elegance_wire` green — round-trip
    set→read, and **a context never told defaults to `LiquidGlass`** (this is
    the proof that any host which forgets to publish keeps today's behaviour,
    R10). `cargo build -p cobolt-forms` green.

- [x] **T3 — Regression baseline harness** (R10; guards AC8, AC10)
  - Files: `crates/cobolt-forms/src/paint.rs` (`#[cfg(test)]`)
  - Do: generalise the existing `ctx.run_ui` + tessellated-shape-count approach
    (cf. `shape_leaf_count`, paint.rs:~8811) into a reusable helper that, for a
    fixture form covering the R4 controls, captures the painted shape list
    under **Liquid Glass** and under **an asset-pack theme**, across **all four**
    `GlassStyle` values. Record the counts as the baseline.
  - Do **not** change any painting code in this task.
  - Verify: `cargo test -p cobolt-forms elegance_baseline` green. Test
    **reports** a table: theme × GlassStyle × shape count (8 rows). These
    numbers are the AC10 contract — T4 must not move them.
  - ⚠️ Honest scope: this is a **structural** proxy for "pixel-identical", not a
    pixel diff. M-6 in the finalize task is what actually confirms the look.

- [x] **T4 — The shared seam (R13)** (R13; AC10)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: add `draw_surface_auto(...)` dispatching on `active_form_style`, and
    rewire **exactly these seven** sub-element sites to call it:
    1446 (non-visual card), 1961 (Shape), 2800 (ProgressBar fill), 3534
    (CheckBox tick box), 4522 (PictureBox frame), 4948 (`glass_combo_header`),
    5012 (`glass_combo_popup`).
    Implement **only** the pass-through arm — every style still reaches the
    existing `draw_glass_auto` call unchanged. The Elegance arm is written in
    T7. Do **not** implement the asset-pack arm (that stays spec 007 T15–T17).
  - Verify: `cargo test -p cobolt-forms elegance_baseline` — **identical counts
    to T3**, all 8 rows. `cargo test -p cobolt-forms` + `cargo build -p
    cobolt-ide` green. This is a pure refactor: any count that moves is a bug.

- [x] **T5 — Selection wiring, end to end** (R2, R3; AC1, AC2)
  - Files: `crates/cobolt-ide/src/app.rs` (`publish_theme_choices` ~1281,
    `resolve_theme_pack` ~1295, preview publish ~11700),
    `crates/cobolt-ide/src/panels/designer.rs` (~4470),
    `crates/cobolt-form-host/src/host.rs` (~872),
    `crates/cobolt-cli/src/form_gui.rs` (~339),
    `crates/cobolt-compiler/src/lib.rs` (`wanted_theme_ids` ~1219, ~1538)
  - Do: replace the hard-coded `vec![(LIQUID_GLASS, "Liquid Glass")]` in
    `publish_theme_choices` with an enumeration of `ThemeCatalog::builtin()`,
    then the discovered packs — so **both pickers gain Elegance with no picker
    code change**. Add a resolver mapping the effective id → `FormStyle`, and
    publish `set_form_style` in all three per-frame host blocks. In the
    compiler, make `wanted_theme_ids` drop **procedural ids generally** (by
    kind, not a hard-coded pair) so `elegance` never triggers a phantom
    `assets/themes/elegance/` lookup or a spurious "falling back to Liquid
    Glass" warning.
  - Verify: `cargo test -p cobolt-compiler elegance_wanted_ids` green (asserts
    no procedural id is requested as a pack). `cargo build --workspace` green.
    **Manual:** launch the IDE → "Elegance" appears in the project-default
    picker *and* the per-form Appearance override (AC1); selecting it
    re-renders immediately (AC2) and the form still looks like Liquid Glass —
    correct at this stage, painting lands in T7.

## Phase 2 — Elegance painting: the shared painter (broad coverage)

- [x] **T6 — Palette helper** (R5)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: `elegance_palette()` returning `elegance::Palette::slate()` plus the
    shape constants from `elegance::Theme::slate()` (`control_radius`,
    `card_radius`, `control_padding_x/y`). **One** place; every later task
    takes colours from here and uses **no colour literals** (this is the
    discipline that keeps the doubled painters in T10–T12 agreeing — plan R-1).
  - Verify: `cargo test -p cobolt-forms elegance_palette` green — asserts the
    helper matches `elegance::Palette::slate()` field-for-field, so a future
    crate upgrade that shifts the palette **fails loudly** instead of drifting.

- [x] **T7 — Elegance faces through `draw_control` + the seam's Elegance arm**
      (R4, R5, R8, R12; AC3 partial, AC6, AC9)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: add the Elegance branch to `draw_control`'s frame chain — **after** the
    asset-pack branch (3043), **before** `else if glass` (3109) — painting from
    `elegance_palette()`. Fill in `draw_surface_auto`'s Elegance arm (T4's
    seam). Explicit `BackgroundColor`/`ForegroundColor` must still win (R8).
    `GlassStyle` must have **no** effect on this path (R12).
  - This single task should visibly convert, on **both** surfaces at once:
    Label, Panel, GroupBox, Line, Shape, Button, CheckBox, RadioButton,
    TextBox frame, Slider, TabControl, ProgressBar, PictureBox, DateTimePicker
    face.
  - Verify: `cargo test -p cobolt-forms elegance_baseline` — Liquid Glass and
    asset-pack rows **still unchanged** (AC8). New test `elegance_glassstyle_
    invariant`: under Elegance the shape list is **identical across all four**
    `GlassStyle` values (AC9), including the CheckBox tick box — the
    sub-element that motivated R13. New test `elegance_explicit_colors`:
    a control with `BackgroundColor` set paints that colour (AC6).
    **Manual:** designer canvas and Preview both show the Elegance look at the
    control's exact designed geometry.

- [x] **T8 — Chart data marks** (R4; AC3 partial)
  - Files: `crates/cobolt-forms/src/paint.rs` (chart-style hook ~5821–5843)
  - Do: the chart palette currently resolves from the asset pack, else the
    built-in accent list. Add the Elegance case so the six chart types take
    their data-mark palette and stroke width from `elegance_palette()` — the
    same treatment spec 007's R7 gave asset packs. Monochrome mode (spec 013)
    must keep working.
  - Verify: `cargo test -p cobolt-forms elegance_chart_palette` green —
    asserts chart marks resolve to Elegance colours under Elegance and are
    **unchanged** under Liquid Glass / asset packs. **Manual:** a form with
    BarChart + LineChart + PieChart under Elegance.

- [x] **T9 — Install the real theme for the spec-039 widgets** (R6; AC4)
  - Files: `crates/cobolt-form-host/src/host.rs` (~872),
    `crates/cobolt-ide/src/app.rs` (~11700),
    `crates/cobolt-ide/src/panels/designer.rs` (~4470)
  - Do: when Elegance is active, call `elegance::Theme::slate().install(ctx)`
    in the same per-frame block as `set_form_style` (documented as cheap —
    early-returns when unchanged).
  - ⚠️ **Risk check is part of this task (plan R-5/R-6):** `install` calls
    `ctx.global_style_mut`, and in the **IDE** that `Context` is shared with the
    IDE's own chrome. Verify the IDE's panels, buttons and code editor are
    visually unaffected, and that the editor's glyphs still render (the install
    also registers a fallback font). **If it bleeds into IDE chrome, do not
    force it** — scope the install to the form viewport, or drop it on the IDE
    surfaces and rely on the crate's documented `Theme::slate()` fallback,
    which is exactly today's behaviour. Record which route was taken.
  - Verify: `cargo build --workspace` green. **Manual (AC4):** a form with
    Knob + Gauge + Switch + FileDropZone under Elegance — those four share the
    palette with everything around them, and their glyphs are not tofu. **And**
    the IDE around the canvas looks untouched.

## Phase 3 — The doubled painters (plan R-1: the highest-risk work)

> Each control below has **two** independent implementations — a static face in
> `paint.rs::draw_control` and a live one in `render.rs`. Both must be themed,
> and the pair verified side by side. Take every colour from
> `elegance_palette()` (T6); **no colour literals** — that discipline is what
> keeps the two in agreement.

- [x] **T10 — ComboBox · ListBox · NumericUpDown** (R4; AC3 partial)
  - Files: `crates/cobolt-forms/src/render.rs` (ComboBox ~3673, ListBox ~3709,
    NumericUpDown ~3648), `crates/cobolt-forms/src/paint.rs` (their static
    faces; ComboBox's header/popup helpers already route through T4's seam)
  - Verify: `cargo test -p cobolt-forms` green. **Manual (parity):** designer
    canvas beside the running form for each of the three — they match,
    including the open ComboBox popup.

- [x] **T11 — MenuBar · ToolBar · StatusBar · Splitter** (R4; AC3 partial)
  - Files: `crates/cobolt-forms/src/render.rs` (MenuBar ~5599, ToolBar/StatusBar
    ~5855, Splitter ~5578), `crates/cobolt-forms/src/paint.rs` (static faces)
  - Do: includes MenuBar's open pulldown (a real `egui::Area` + `Frame::popup`
    with hard-coded colours today).
  - Verify: `cargo test -p cobolt-forms` green. **Manual (parity):** each of the
    four matches between canvas and running form; the MenuBar pulldown is
    themed, not default-egui.

- [x] **T12 — DataGrid · TreeView** (R4; AC3 partial)
  - Files: `crates/cobolt-forms/src/render.rs` (DataGrid ~3878, TreeView ~5517),
    `crates/cobolt-forms/src/paint.rs` (static faces)
  - Do: the two heaviest — DataGrid hand-paints header, rows, cells, grid lines,
    scrollbars, sort indicators and a filter `TextEdit`; TreeView hand-paints
    rows and indentation. Alternating-row tint, selection and grid lines all
    come from the palette.
  - Verify: `cargo test -p cobolt-forms` green. **Manual (parity):** canvas vs
    running form for a populated DataGrid (multiple columns, a selected row, an
    active sort) and a nested TreeView.

## Phase 4 — Proof, docs, finalize

- [x] **T13 — Coverage + naming proof** (R7, R9, R11; AC5, AC7)
  - Files: `crates/cobolt-forms/src/paint.rs` / `render.rs` (`#[cfg(test)]`)
  - Do: a test that walks **every `ControlType` named in R4** and asserts the
    Elegance path is taken — so a family left on the Liquid Glass fallback
    **fails the build** rather than shipping quietly (R11). Separately, prove
    R7 still degrades gracefully using a synthetic unmapped kind.
  - Verify: `cargo test -p cobolt-forms elegance_coverage` green. Test
    **reports** a covered/total tally **listing each control by name** — the
    "which ones", not just a count (steering: quantified, human-readable
    results). Plus AC7:
    ```bash
    grep -rn "egui-elegance\|elegance::" docs/ crates/cobolt-ide/src/i18n.rs crates/cobolt-codegen/src/ ; echo "exit=$? (1 = clean)"
    ```

- [x] **T14 — Docs & i18n** (R9; AC7)
  - Files: `docs/developers-guide-en.md` (extend "Form themes and styles",
    ~line 1464), `crates/cobolt-ide/src/i18n.rs` (**only if** a new string
    appeared)
  - Do: document Elegance in the existing themes section — what it is, how to
    select it (project default / per-form override), and that it is a
    control-chrome theme with no themed background. Use **"Elegance" only** —
    never the crate name (R9). English guide only; **never** touch
    `-es/-pt/-jp/-cn`. Explain in COBOL/developer terms, not Rust.
  - Verify: `cargo test -p cobolt-ide i18n` green (no empty translations). Plan
    expects **no new `Tr` keys** — "Elegance" is a product term carried in
    `display_name`. If one did appear, it is all six languages.

- [x] **T15 — Finalize** (all ACs)
  - Files: `crates/cobolt-ide/src/version.rs` (`1.60.49` → **`1.61.0`** — minor
    bump, operator-authorised), `CHANGELOG.md`
  - Do: version + changelog entry. Run the System KB freshness check; no
    control/property/method/event changed, so it is expected green — **if it
    goes red, that is a real failure**, fix it in this change (rebuild
    `assets/knowledge/chunked.data`), do not wave it through.
  - Verify: `cargo build --workspace` + `cargo test --workspace` green.
    Then the **manual AC walkthrough** (plan §6 M-1…M-7):
    - M-1 AC2/AC3 — every R4 control under Elegance, both surfaces
    - M-2 **R-1 parity** — the eight doubled controls, canvas vs running form
    - M-3 AC4 — the four spec-039 widgets share the palette
    - M-4 AC6 — explicit control colours still win
    - M-5 AC9 — cycling all four `GlassStyle` values changes **nothing**
    - M-6 AC8 — an untouched Liquid Glass form looks unchanged; IDE chrome and
      editor glyphs unaffected
    - M-7 R5 parity — `rcrun run-form` on an Elegance form matches the IDE
  - Commit discipline: **feature only**, its own commit, never mixed with a
    fix. Announce on **f=96** with the `[Noticia]` prefix (not f=97), in
    Spanish BBCode, signed "Anthropic Claude Codex Agent" — **after** merge to
    main, and **only** when the operator asks. Respect the push window.

---

## Acceptance-criteria coverage

| AC | Covered by |
|----|-----------|
| AC1 — Elegance in both pickers | T5 |
| AC2 — immediate re-render on select | T5, T15 (M-1) |
| AC3 — every R4 control, both surfaces | T7, T8, T10, T11, T12, T13 |
| AC4 — spec-039 widgets share the palette | T9 |
| AC5 — full coverage; fallback degrades gracefully | T13 |
| AC6 — explicit control colours win | T7 |
| AC7 — no crate name user-facing | T13, T14 |
| AC8 — existing forms unchanged | T3, T4, T7, T15 (M-6) |
| AC9 — `GlassStyle` inert under Elegance | T7, T15 (M-5) |
| AC10 — seam pass-through across 4 styles | T3, T4 |

Requirements: R1 (T1) · R2 (T1, T5) · R3 (T5) · R4 (T7, T8, T10–T12) ·
R5 (T6, T7) · R6 (T9) · R7 (T13) · R8 (T7) · R9 (T13, T14) · R10 (T3, T4, T15) ·
R11 (T13) · R12 (T2, T7) · R13 (T4)

## Done criteria

All ten acceptance criteria checked, `cargo test --workspace` green, the English
guide updated (translations untouched), version at 1.61.0 with a CHANGELOG
entry, and the work staged as a **feature** commit — separate from any fix, not
committed or pushed unless the operator asks.

**No partial-coverage ship point (R11):** every control family in R4 must be
covered before this is called done. T13 is the gate that enforces it.
