# Spec — Chart monochrome mode

- **Status:** draft → approved
- **Folder:** specs/013-chart-monochrome-mode/
- **Author:** Eslopes (with Anthropic Code Agent)   **Date:** 2026-06-20

## 1. Overview

Add an optional **Monochrome** mode to all chart controls. When enabled, a chart
renders its data using **tonal variations of one user-selected base colour**
instead of the standard multi-colour palette. Supporting elements (grid, axes,
borders) use softer/pastel or lighter/darker variants of that colour, while text
(labels/legends/titles) keeps the existing foreground colour. The base colour is
chosen from a predefined **256-colour** set that excludes pure black/white.

## 2. Goals / Non-goals

### Goals
- New chart props: `Monochrome` (bool, default false) and `MonochromeColor`
  (colour, default a medium blue). Grid visibility is the **existing
  `ShowGridLines`** prop (reused — see Q1).
- A `GenerateMonochromePalette(base, count)` helper producing `count`
  distinguishable same-hue tones (avoiding pure black/white).
- Distinct colour **roles** when monochrome: data tones, border variant, grid
  pastel, axis variant; legend markers = data tones; **text = ForegroundColor**.
- Apply across **all six** chart types (Bar, Line, Pie, Area, Scatter, Donut).
- A **256-colour selector** for `MonochromeColor` excluding `#000000`/`#FFFFFF`
  and near-black/near-white.

### Non-goals
- Changing the multi-colour palette path (`Monochrome = false` is unchanged).
- Re-theming chart **text** (it already uses/should use the foreground colour;
  monochrome must not touch it).
- Changing the existing **transparency** logic (area/stacked alpha unchanged).
- A free-form colour wheel (selection is the fixed 256 set).

## 3. User stories
- As a developer, I tick **Monochrome** on a chart, pick a blue from the
  256-swatch grid, and the whole chart renders in distinguishable blues.
- As a developer, grid/axis lines become soft pastel blues, not the default
  multi-colour scheme, and my labels stay readable in the foreground colour.

## 4. Requirements (EARS)

- **R1:** Chart controls shall expose `Monochrome` (bool, default false) and
  `MonochromeColor` (colour, default medium blue).
- **R2:** Grid visibility shall be controlled by the existing `ShowGridLines`
  (default true); when off, no grid lines render. (Satisfies the "ShowGrid"
  requirement without a duplicate property — Q1.)
- **R3 (state — Monochrome off):** the chart shall render exactly as today (theme
  / `SeriesColors` / default palette).
- **R4 (state — Monochrome on):** the standard data palette shall be ignored;
  data elements (bars/slices/lines/points/areas/markers) shall each use a
  **distinguishable tonal variation** of `MonochromeColor` (same hue family).
- **R5:** supporting elements shall use *softer* variants of `MonochromeColor`:
  **grid lines = soft pastel**, **axis lines = pastel/slightly stronger**; they
  shall not be the same tone as the data elements.
- **R6:** data-element **borders/outlines** shall use a lighter-or-darker variant
  of `MonochromeColor` (lighter on a dark chart background, darker on a light one).
- **R7 (constraint):** when `Monochrome = true`, **text** (axis/data/value labels,
  legend text, titles, captions, tooltips) shall **not** be recoloured by the
  monochrome palette — it keeps `ForegroundColor`.
- **R8 (constraint):** existing **transparency** behaviour (area/stacked alpha)
  shall be unchanged; monochrome only changes the *hue* of the fill, not its alpha.
- **R9:** the renderer shall use a helper `monochrome_palette(base, count)`
  returning ≥`count` tones that stay in-hue, vary saturation/brightness enough to
  distinguish, avoid pure black/white, and keep readable contrast; softer/pastel
  variants are derived for support roles.
- **R10:** `MonochromeColor` shall be chosen from a fixed set of **256** colours
  that excludes `#000000`, `#FFFFFF`, and colours too close to either.
- **R11 (constraint):** the feature shall work for **BarChart, LineChart,
  PieChart, AreaChart, ScatterChart, DonutChart**.
- **R12 (constraint):** new UI strings → `Tr` ×6 if added (else follow the chart
  pane's existing inline-literal convention); `.cfrm` round-trips the new props;
  generated-code/regenerate contract unaffected.

## 5. Acceptance criteria
- [ ] **AC1** — charts expose a `Monochrome` checkbox + a `MonochromeColor`
  selector of **256** colours; `#000000`/`#FFFFFF` absent (R1, R10).
- [ ] **AC2** — `Monochrome = false` → rendering is byte-for-byte the prior
  behaviour (R3).
- [ ] **AC3** — `Monochrome = true` → every data element uses a variation of the
  base colour and adjacent elements are visually distinguishable (R4, R9).
- [ ] **AC4** — grid lines use a soft pastel of the base; `ShowGridLines = false`
  hides them (R2, R5).
- [ ] **AC5** — borders use a lighter/darker variant; axes use a pastel/stronger
  variant (R5, R6).
- [ ] **AC6** — labels/legends/titles keep `ForegroundColor`; area/stacked alpha
  unchanged (R7, R8).
- [ ] **AC7** — all six chart types honour monochrome (R11); `monochrome_palette`
  has unit tests (in-hue, no pure black/white, distinct tones) (R9).
- [ ] **AC8** — `.cfrm` round-trips `Monochrome`/`MonochromeColor`; i18n green
  (R12).

## 6. Constraints & steering check
- **Fix vs feature:** functionally a feature, but **treated as a fix** per the
  operator's pre-production directive → patch (`z`) bump + CHANGELOG, **f=97**.
- **i18n:** chart-pane labels use inline literals (existing convention); no new
  `Tr` keys expected.
- **Docs:** add a short monochrome note to the English guide chart section.
- **No "cobolt" in user text; COBOL identifiers English.**

## 7. Open questions
- **Q1 (ShowGrid vs ShowGridLines): RESOLVED — reuse `ShowGridLines`.** It already
  exists, defaults true, and gates the grid; adding a second `ShowGrid` prop would
  duplicate it. The spec's grid-visibility requirement maps onto `ShowGridLines`.
- **Q2 (default MonochromeColor):** a medium blue, e.g. `#3F6FB5`. Confirm in /plan.
- **Q3 (256-set construction):** a 16×16 HSL grid (16 hues × 16 saturation/
  lightness steps) with lightness bounded to ~[0.22, 0.80] so no swatch is near
  black/white. Confirm in /plan.
