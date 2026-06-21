# Tasks — Chart monochrome mode

- **Status:** in progress   **Plan:** ./plan.md   **Date:** 2026-06-20

- [x] **T1 — Model props** (R1, R2)
  - `cobolt-forms/src/model.rs`: add `Monochrome` (Bool false) + `MonochromeColor`
    (String `#3F6FB5`) to the chart block; reuse `ShowGridLines` for grid.
  - Verify: `cargo test -p cobolt-forms --lib`; test asserts both props exist on
    all 6 chart types and default off / medium blue; non-charts lack them.

- [x] **T2 — Colour helpers** (R9, R10, R5, R6)
  - `cobolt-forms/src/paint.rs`: RGB↔HSL; `monochrome_palette(base,count)`,
    `pastel_of`, `axis_variant`, `border_variant(base,dark_bg)`,
    `chart_palette_256()`.
  - Verify: `cargo test -p cobolt-forms --features render`; unit tests — palette
    length≥count, no pure black/white, in-hue, adjacent tones differ; 256-set is
    len 256, unique, excludes `#000000`/`#FFFFFF` + near-extremes.

- [x] **T3 — Renderer monochrome branch** (R3, R4, R5, R6, R7, R8, R11)
  - `draw_chart_preview`: when `Monochrome`, replace `pal`, `grid_c`, axis colour,
    and data-mark borders with derived roles; index per element across all 6 chart
    types; keep area alpha + text paths unchanged. Off ⇒ unchanged.
  - Verify: `cargo build -p cobolt-forms --features render` + `cargo build -p
    cobolt-ide`; manual per-type check (T7).

- [x] **T4 — Properties pane: checkbox + 256 picker** (R1, R10)
  - `cobolt-ide/src/panels/properties.rs`: `Monochrome` checkbox in the chart
    Visual section; when on, a 256-swatch grid (from `chart_palette_256()`) sets
    `MonochromeColor` + a current-colour preview.
  - Verify: `cargo build -p cobolt-ide`; picking a swatch updates the prop;
    `cargo test -p cobolt-ide i18n`.

- [x] **T5 — Serialization round-trip test** (R12)
  - `cobolt-forms` test: a chart with `Monochrome=true`+`MonochromeColor` saves &
    reloads identically.
  - Verify: `cargo test -p cobolt-forms --lib`.

- [x] **T6 — Docs + finalize** (R12)
  - English guide chart section: monochrome paragraph. Patch version bump (fix) +
    CHANGELOG. Full `cargo test --workspace`.
  - Verify: workspace green; guide updated.

- [ ] **T7 — Manual AC walkthrough** (AC1–AC8)
  - `cargo run -p cobolt-ide`: tick Monochrome on each chart type, pick swatches,
    confirm distinguishable data tones, pastel grid, lighter/darker borders,
    unchanged labels; toggle `ShowGridLines`; Monochrome off → original.

## AC coverage
AC1→T1,T4 · AC2→T3 · AC3→T2,T3 · AC4→T2,T3 · AC5→T2,T3 · AC6→T3 · AC7→T2,T3,T7 · AC8→T5,T6
