# Plan — Chart monochrome mode

- **Status:** approved   **Spec:** ./spec.md   **Date:** 2026-06-20

## 1. Approach
Add two chart props and a colour-role layer in the shared chart renderer. When
`Monochrome` is off, nothing changes (R3). When on, the data palette is replaced
by `monochrome_palette(base, count)` and support colours (grid/axis/border) are
derived variants; text and transparency are untouched (R7, R8).

- **Model** (`cobolt-forms/src/model.rs`): in the chart prop block add
  `Monochrome` (Bool false) and `MonochromeColor` (String `#3F6FB5`). Reuse
  existing `ShowGridLines` for grid visibility (R1, R2, Q1).
- **Colour helpers** (`cobolt-forms/src/paint.rs`): RGB↔HSL, then
  - `monochrome_palette(base, count) -> Vec<Color32>` — spread `count` tones over
    a saturation/lightness gradient at the base hue, lightness clamped to
    ~[0.20, 0.82] (no pure black/white), enough spread to distinguish (R9).
  - `pastel_of(base)` (grid), `axis_variant(base)` (axes), `border_variant(base,
    dark_bg)` (outlines: lighter on dark, darker on light) (R5, R6).
  - `chart_palette_256() -> Vec<Color32>` — the 16×16 HSL selector set, lightness
    bounded so `#000000`/`#FFFFFF` and near-extremes are excluded (R10, Q3).
- **Renderer** (`draw_chart_preview`): read `Monochrome`/`MonochromeColor`; if on,
  set `pal = monochrome_palette(base, n)` (n = elements that chart type needs),
  `grid_c = pastel_of`, `ax_c = axis_variant`, slice/bar/area/marker stroke =
  `border_variant(base, dark_bg=!hide_bg)`. Index per element so adjacent
  bars/slices/markers differ (R4). Area fill keeps its current alpha, hue from the
  palette (R8). Text paths unchanged (R7).
- **Properties pane** (`cobolt-ide/src/panels/properties.rs`): in the chart
  Visual section add a `Monochrome` checkbox; when ticked, show a **256-swatch
  grid** (from `chart_palette_256()`) that sets `MonochromeColor`, plus a preview
  of the current base. `ShowGridLines` checkbox already present.
- **Docs**: English guide chart section — short monochrome paragraph.
- **Version**: patch bump (fix per directive) + CHANGELOG; **f=97** announcement.

## 2. Affected files
- `crates/cobolt-forms/src/model.rs` — 2 props + test.
- `crates/cobolt-forms/src/paint.rs` — colour helpers + monochrome branch in
  `draw_chart_preview` + unit tests.
- `crates/cobolt-ide/src/panels/properties.rs` — checkbox + 256 picker.
- `docs/developers-guide-en.md`; `version.rs` + `CHANGELOG.md`.

## 3. Data / model changes
- `Monochrome: Bool=false`, `MonochromeColor: String="#3F6FB5"` on the six chart
  types only. Serde/`.cfrm` round-trip is automatic (generic prop map).
  Back-compat: absent → defaults → unchanged rendering.

## 4. Key decisions
- **Reuse `ShowGridLines`** for grid visibility (don't add `ShowGrid`) — avoids a
  duplicate toggle. (Q1)
- **HSL tonal generation** — predictable in-hue spread; clamp lightness to avoid
  black/white and keep contrast. Rejected: random jitter (not reproducible) and
  RGB scaling (drifts hue/þblackens).
- **Colour roles, not one palette** — data vs grid/axis/border get distinct
  derived tones so the chart isn't flat (R5).
- **Text & alpha untouched** — monochrome only sets hues of marks/support lines.

## 5. Risks & mitigations
- **256 set must exclude black/white & near-extremes** → lightness-bounded HSL
  grid; unit test asserts no `#000000`/`#FFFFFF` and a min distance from extremes.
- **Distinguishability with many elements** (e.g. 10 bars) → palette sized to the
  element count with even lightness spread; unit test checks adjacent tones differ.
- **Hue drift / dull tones** → operate in HSL, keep S reasonable; test hue stays
  within a small delta of the base hue.

## 6. Test strategy
- **cobolt-forms unit:** `monochrome_palette` (count length, in-hue, no pure
  black/white, monotone-distinct); `chart_palette_256` (len 256, unique, excludes
  `#000000`/`#FFFFFF` + near-extremes); chart props default off / medium blue.
- **`.cfrm`:** round-trip a chart with `Monochrome=true` + a `MonochromeColor`.
- **Manual:** in the IDE, tick Monochrome on each of the 6 charts, pick a swatch,
  confirm data tones distinguishable, grid pastel, borders lighter/darker, labels
  unchanged; toggle `ShowGridLines`; toggle Monochrome off → original look.

## 7. Steering compliance
- [ ] Fix (pre-prod directive) → patch bump + CHANGELOG; f=97.
- [ ] i18n: inline literals per the chart-pane convention (no new Tr keys).
- [ ] English guide updated; translations untouched.
- [ ] No "cobolt" in user text; COBOL source English.
- [ ] Generated-code/regenerate contract unaffected.
