# Plan — Chart multi-series data and interaction

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-08-18

## 1. Approach

Four seams, in dependency order. Each is independently testable, and the first
three are pure data plumbing that leaves the picture unchanged until the fourth
draws with it.

### 1.1 A series-shaped wire format (R1, R5, R19, R20)

A chart's live data reaches the renderer as the control's `__ChartData`
property — one `label<TAB>value` line per point, written by
`Interpreter::push_chart_data`. That form stays exactly as it is.

Multi-series data travels in a **new, separate** property, `__ChartSeries`:

```
<TAB>Net<TAB>Tax          ← header: empty first cell, then one name per series
Jan<TAB>120<TAB>18        ← one line per category: label, then one value per series
Feb<TAB>140<TAB>21
```

A new property rather than an extended `__ChartData` because R5/R19 demand that
a chart fed by `COBOL-CHART-ADD-POINT` draw *byte-identically* to 1.61.97. A
format sniff on the existing property would put that guarantee at the mercy of a
label that happens to contain a tab; a separate key cannot regress what it does
not touch.

### 1.2 One resolver, four sources (R1–R4, R6, R11)

New `crates/cobolt-forms/src/chart_data.rs`:

```rust
pub struct ChartSeries { pub name: String, pub values: Vec<f32> }
pub struct ChartData {
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,   // R6 — every series given, no cap
    pub bubble: Option<Vec<f32>>,   // R9
    pub is_sample: bool,            // the designer's representative preview
}
impl ChartData {
    /// The value range the axis must span, always including zero (R21).
    /// `(min, max)` with `min <= 0.0 <= max`.
    pub fn value_range(&self) -> (f32, f32);
}
pub fn resolve(ctrl: &Control) -> ChartData
```

Precedence, highest first:

1. `__ChartSeries` — a multi-series push from the runtime.
2. `__ChartData` — the existing single-series push. Yields exactly one series,
   named from `SeriesLabels`' first entry when present, else the control's
   `Title`, else `Series 1`.
3. **Property fallback (R3)** — no live data: `ValueFields` names the series,
   `SeriesLabels` names them for the legend, `LabelField` names the category
   field. This is the *design-time shape* preview, drawn with sample values so
   an unbound chart still looks like what it is configured to be.
4. The existing representative sample (`is_sample = true`).

**R4 is satisfied structurally, not by a comparison.** The binding is the
authority because a resolved binding *produces* levels 1 and 2, which outrank
the properties at level 3. The painter never has to arbitrate; it receives
resolved series. This is why R4 needs no runtime check and cannot drift.

**No cap (R6).** Every series given is carried through. The palette is already
indexed `pal[i % pal.len()]`, so more series than colours repeats rather than
panics or drops — nothing to add, and nothing to truncate.

### 1.2b The zero baseline (R21, R22, R23)

Today the plot's floor *is* zero: `px_y(v) = plot.max.y - v * plot.height()` with
`v` normalised `0..1` against the maximum. A negative value maps below the floor
and is clipped away.

`ChartData::value_range()` returns `(min, max)` with `min <= 0 <= max`, and the
painter derives a **zero line**:

```
zero_y = plot.max.y - (0 - min) / (max - min) * plot.height()
px_y(v) = plot.max.y - (v - min) / (max - min) * plot.height()
```

For an all-positive chart `min == 0`, so `zero_y == plot.max.y` and `px_y`
reduces to exactly today's formula — **R23 holds by algebra**, not by a special
case, which is the only way to be sure an all-positive chart does not shift by a
pixel. Bars are drawn from `zero_y` to `px_y(v)` in whichever direction that
runs; the X axis line moves from the plot floor to `zero_y`.

### 1.3 Producing series (R2, R3)

- **Runtime (`cobolt-runtime`).** `Interpreter::chart_data` becomes
  `HashMap<String, ChartPoints>` where `ChartPoints` holds category labels and N
  named series. `push_chart_data` writes `__ChartData` when there is exactly one
  unnamed series (unchanged bytes) and `__ChartSeries` otherwise. A new
  `refresh_chart_binding(control_id)` mirrors the existing
  `refresh_datagrid_binding`: it reads the control's `_BindingKind` /
  `_BindingFields` object properties and builds one series per
  `ChartValueSeries` mapping, categories from `ChartCategory`, names from
  `ChartSeriesLabel`.
- **New CALL, not a changed one (R20).** `COBOL-CHART-SET-SERIES` accepts a
  series name and a table; `COBOL-CHART-ADD-POINT` and `COBOL-CHART-SET-TABLE`
  keep their signatures and their single-series meaning.
- **Design time (`cobolt-ide`).** `refresh_data_binding_target_properties` gains
  a `BindingTargetDescriptor::Chart` arm mirroring its existing `DataGrid` arm —
  which already writes `Columns` / `DataSource` / `Rows` onto a bound grid. For a
  chart it writes a preview `__ChartSeries` so the **canvas** shows the bound
  shape, exactly as a bound grid shows its columns.

### 1.4 Drawing and interaction (R7–R18)

All in `paint.rs::draw_chart_preview`, which already received the seven visual
properties in 1.61.97.

- **Stacking (R7, R8, R22):** bar and area only. Two running totals per
  category — one upward from zero for the positive contributions, one downward
  for the negative — so a category holding both shows both, and no value is
  discarded. Line/Scatter/Pie/Donut ignore `Stacked`; no branch is added for
  them, so R8 holds by construction.
- **Bubbles (R9, R10):** when `ChartData::bubble` is present, a scatter point's
  radius interpolates from a visible floor to `BubbleScale` across the field's
  range; absent, every point keeps `PointRadius`.
- **Pie/Donut (R12):** take `series[0]` only.
- **Legend (R11):** names come from `ChartData::series[i].name`. The `Series N`
  placeholder survives only as the level-2/4 fallback.
- **Interaction without a `Ui` (R18).** The chart is drawn from an
  `egui::Painter`, and `painter.ctx()` supplies everything R13–R17 need:
  `ctx.input(|i| i.time)` for the clock, `ctx.pointer_hover_pos()` for the
  pointer, `ctx.request_repaint()` to keep an animation running. **No signature
  change**, so every existing caller — canvas, preview, run, compiled binary —
  gets the behaviour at once, which is what R17 asks for.
- **Tooltip (R13, R14):** the chart records each element's hit rect as it draws,
  then paints its own rounded box + text for the element under the pointer. Self
  painted, so it is identical on all four surfaces and inside the compiled
  binary, where egui's `show_tooltip` (which needs a `Ui` and a `Response`) is
  not reachable.
- **Animation (R15, R16):** the trigger is **new data**, not the first draw
  (operator, Q2). `ctx.data` holds `(data_fingerprint, start_instant)` per
  control id. Each frame the chart fingerprints its resolved data (a cheap hash
  of categories + series names + values); when the stored fingerprint differs,
  the animation restarts and the new pair is stored. Progress
  `t = ((now - start) / DURATION).clamp(0,1)` scales the distance from the zero
  line, so a negative bar grows downward as a positive one grows upward. While
  `t < 1`, `request_repaint()`.

  Fingerprinting the *data* rather than the control gives R16 for free: moving,
  resizing, hovering, re-selecting or restyling a chart leaves the fingerprint
  untouched, so nothing replays. It also removes the Q2 worry about an animation
  restarting while the developer drags a chart around the canvas.

## 2. Affected crates / files

| File | Change |
|---|---|
| `crates/cobolt-forms/src/chart_data.rs` | **New.** `ChartData`, `ChartSeries`, `resolve()`, `value_range()`. No cap. |
| `crates/cobolt-forms/src/lib.rs` | Register the module, re-export the types. |
| `crates/cobolt-forms/src/paint.rs` | `draw_chart_preview`: consume `ChartData`; stacking; bubbles; series-named legend; tooltip; animation. Re-bless `elegance_baseline_tests`. |
| `crates/cobolt-runtime/src/interpreter.rs` | `chart_data` becomes series-aware; `push_chart_data` picks `__ChartData` vs `__ChartSeries`; new `refresh_chart_binding`; new `COBOL-CHART-SET-SERIES`. |
| `crates/cobolt-ide/src/app.rs` | `refresh_data_binding_target_properties`: add the `Chart` arm. |
| `crates/cobolt-compiler/src/lib.rs` | KB tables: rewrite all 8 property entries; document `COBOL-CHART-SET-SERIES`; note the series cap. |
| `assets/knowledge/chunked.data` | Regenerated in the same change. |
| `docs/developers-guide-en.md` | Multi-series charts, stacking, bubbles, tooltips, animation. English only. |
| `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md` | Fix-number bump + entry. |
| `crates/cobolt-ide/src/i18n.rs` | **Only if** an IDE-side string appears (see §7). |

## 3. Data / model changes

- **No `.cfrm` schema change.** All eight properties already exist on the
  control and already serialise. Forms saved before this feature load unchanged.
- **New runtime→GUI property `__ChartSeries`.** Transient, like `__ChartData` —
  pushed as a `StateUpdate`, never written to the `.cfrm`.
- **`Interpreter::chart_data` value type changes** (internal; no COBOL-visible
  effect). Existing `COBOL-CHART-*` calls map onto the one-unnamed-series case.
- **Migration:** none required. A chart with no `__ChartSeries` behaves exactly
  as today, which is what R5/R19 demand.

## 4. Key decisions & alternatives

- **Decision:** a separate `__ChartSeries` property. **Why:** guarantees the
  single-series path is untouched, which R5/R19 require and AC4 tests.
  **Rejected:** extending `__ChartData` with a header line — a sniff that a
  tab-bearing label could defeat, putting a non-regression guarantee at risk for
  no gain.
- **Decision:** precedence expressed as *source order in one resolver*.
  **Why:** makes R4 ("binding wins") structural — the properties sit at a lower
  level than the data a binding produces, so they cannot override it and the
  rule cannot rot. **Rejected:** a `has_binding` check in the painter — the
  painter has a `Control`, not a `Form`, so it cannot see bindings at all.
- **Decision:** take the clock, pointer and repaint from `painter.ctx()`.
  **Why:** satisfies R17 (every surface) with **no signature change** and no new
  call sites. **Rejected:** giving `draw_chart_preview` a `&mut Ui` — it is
  called from `draw_control`, which the designer canvas invokes from a painter;
  threading a `Ui` through would touch every control type for one control's
  benefit.
- **Decision:** the chart paints its own tooltip. **Why:** R14 — identical on
  four surfaces, and available in the compiled binary. **Rejected:**
  `egui::show_tooltip`, which needs a `Ui` and a `Response`.
- **Decision:** **no series cap** (operator, Q1). **Why:** a cap is a truncation,
  and a chart that quietly drops the ninth series is the same defect class as the
  dropdown that dropped its ninth item (1.61.90). The palette is already indexed
  modulo its length, so repeating colours is the natural degradation.
  **Rejected:** a cap with a visible marker — it solves the silence, not the
  truncation.
- **Decision:** derive a **zero line** from `value_range()` rather than special
  casing negatives. **Why:** for an all-positive chart `min == 0`, so the formula
  collapses to today's exactly — R23 holds by algebra rather than by a branch
  someone could forget. **Rejected:** clamping negatives to zero (the operator
  rejected it outright), and a separate "has negatives" code path, which would
  leave two formulas to keep in step.
- **Decision:** trigger the animation on a **data fingerprint** change.
  **Why:** it is literally R15 ("whenever new data arrives") and it gives R16
  free — nothing about moving, resizing or restyling a chart changes the
  fingerprint. **Rejected:** "first sight of the control id", which the operator
  replaced, and which would have replayed whenever egui evicted the entry.

## 5. Risks & mitigations

- **Risk:** the paint baseline (`elegance_baseline_tests`) moves again, and a
  re-bless hides an unintended change. → **Mitigation:** re-bless only with the
  arithmetic accounted for per control family, as 1.61.97 did; the number must be
  explainable before it is written down.
- **Risk:** the canvas tooltip fights the designer's drag/select (Q3). →
  **Mitigation:** suppress the tooltip while a pointer button is down.
- **Risk:** `ctx.data` is memory that egui may evict, replaying an animation.
  → **Mitigation:** an evicted fingerprint reads as "new data", so the worst case
  is one extra replay, not a wrong picture; accepted and documented.
- **Risk:** the zero line silently shifts an existing all-positive chart by a
  sub-pixel amount, which no shape-count test would catch. → **Mitigation:**
  AC19 compares an all-positive chart's painted geometry **rect by rect** against
  the pre-change capture, not just the shape count.
- **Risk:** a chart whose values are all negative puts the zero line at the plot
  **top**, which looks like a bug until you read the axis. → **Mitigation:**
  correct by R21 and worth a guide note; called out in T14 rather than left for
  the operator to discover.
- **Risk:** the data fingerprint is computed every frame for every chart. →
  **Mitigation:** it hashes at most the resolved values, which the painter has
  already parsed; measure it in the T11 test and report the cost.
- **Risk:** the runtime's `chart_data` type change ripples into existing chart
  tests. → **Mitigation:** land §1.3 behind the unchanged `__ChartData` output
  first, so `test_chart_inline_methods` stays green throughout.

## 6. Test strategy

**`cobolt-forms` — resolver (`chart_data.rs` unit tests)**
- Precedence: `__ChartSeries` beats `__ChartData` beats properties beats sample
  (AC1–AC3). Reports which level each fixture resolved at.
- No cap: more series than the palette has colours ⇒ all of them resolve and the
  palette repeats; none is dropped (AC15).
- `value_range()` always brackets zero: all-positive, all-negative and mixed
  fixtures each report their `(min, max)` (AC17, AC19).

**`cobolt-forms` — painter (`render.rs` test module, `drive_painted`)**
- AC4 non-regression: a chart fed only `__ChartData` produces a painted-shape
  set identical to the pre-change capture. Reports the shape count both ways.
- AC5/AC6 stacking: bar heights sum; area upper edge is the running total.
  Reports the measured bar rects.
- AC7: `Stacked` on line/scatter/pie/donut changes no shape.
- AC8 bubbles: point radii ordered by the bubble field, largest == `BubbleScale`.
  Reports the radii.
- AC9: a pie bound to two series paints one ring's worth of slices.
- AC10/AC11 tooltip: with the pointer inside a bar, a text shape carrying the
  category, series name and value is painted; off, it is not.
- AC12/AC13 animation: capture at t=0 vs t=DURATION shows shorter bars at t=0;
  a repaint is requested while running; a third draw after completion matches
  the second exactly.
- AC14: AC10 and AC12 asserted through **both** the static/canvas path
  (`painted_text`) and the interactive path (`drive_painted`).

**`cobolt-runtime`**
- `COBOL-CHART-ADD-POINT` / `SET-TABLE` still emit `__ChartData` with identical
  bytes (AC4, R20) — extends `test_chart_inline_methods`.
- `COBOL-CHART-SET-SERIES` emits `__ChartSeries` in the documented shape.
- `refresh_chart_binding` builds one series per `ChartValueSeries` mapping with
  names from `ChartSeriesLabel` (AC1).

**`cobolt-ide`**
- `refresh_data_binding_target_properties` writes a preview `__ChartSeries` for
  a bound chart, and does not for an unbound one (AC2).
- `prebuilt_chunked_kb_matches_the_published_documentation` green (AC16).

**Manual / visual** — `cargo run -p cobolt-ide`: drop a chart, bind a table with
two numeric fields, confirm two named series on the canvas; tick `Stacked` and
watch the bars merge; hover a bar in Run Form and read the tooltip; reopen the
form and watch it animate once.

**Reporting** — every new test prints a result block naming the cases exercised
and the measured numbers (bar heights, radii, shape counts), per `tech.md`.

## 7. Steering compliance

- [ ] **i18n:** the work lives in `cobolt-forms`, which is linked into the
      compiled binary and has no access to the IDE's `Tr` table — chart-painted
      text has always been an English literal there. **No new IDE strings are
      planned.** If the dropped-series marker or any inspector hint lands on the
      IDE side instead, it becomes a `Tr` field in all six languages; each task
      that adds a string states which side it is on.
- [ ] **Generated-code banner + regenerate-on-action:** codegen emits the
      existing `COBOL-BINDING-POPULATE` call and mapping comments; the banner and
      the regenerate contract are untouched. Any emitted line keeps them.
- [ ] **English dev guide updated; translations untouched.**
- [ ] **Fix vs feature:** **FIX** *(operator decision, 2026-08-18)* — all eight
      properties were seeded, exposed in the inspector and published in the
      System KB as working, so delivering them makes good a claim already
      published rather than adding a capability. That is the technical-debt
      category CLAUDE.md rule 4 defines as a fix. Branch `fix/chart-multi-series`;
      fix-number `z` bump + `CHANGELOG.md`; announced on **f=97** with **no**
      thread prefix.
- [ ] **No "cobolt" in user-facing text; COBOL identifiers English** — the new
      `COBOL-CHART-SET-SERIES` call name and any generated source stay English.
