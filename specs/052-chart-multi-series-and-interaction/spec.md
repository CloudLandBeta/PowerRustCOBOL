# Spec — Chart multi-series data and interaction

- **Status:** draft → approved
- **Folder:** specs/052-chart-multi-series-and-interaction/
- **Author:** Anthropic Claude Codex Agent (with the operator)   **Date:** 2026-08-18

## 1. Overview

Eight chart properties are seeded on every chart, shown in the inspector and
documented in the System KB, and read by nothing. Six of them —
`ValueFields`, `SeriesLabels`, `LabelField`, `Stacked`, `BubbleField`,
`BubbleScale` — describe **several** Y series drawn from the sub-fields of a
bound table, together with the names to put on them, whether to stack them, and
a third field controlling bubble size. The remaining two — `ShowTooltips`,
`AnimateOnLoad` — need a pointer and a clock.

A chart today receives exactly **one** series: `label<TAB>value` lines pushed
from COBOL into the control's `__ChartData` property. The data-binding model
already speaks multi-series (`BindingTargetPath::ChartCategory`,
`ChartValueSeries { series_id }`, `ChartSeriesLabel { series_id }`) and the
binding panel already maps up to two numeric fields — but nothing between those
mappings and the painter carries more than one series.

This feature closes both gaps: a multi-series transport from binding to painter,
and interaction (hover tooltips, load animation) on every surface a chart is
drawn on.

## 2. Goals / Non-goals

**Goals**

- Carry **N named series** from a resolved data binding through to the painter.
- Honour all eight properties, with a clearly-ordered source of truth.
- Draw stacked bar/area charts and value-sized scatter bubbles.
- Hover tooltips and a load animation on **every** surface: designer canvas,
  Preview, Run Form, and the compiled binary.
- Leave a single-series chart drawing exactly as it does at 1.61.97.

**Non-goals**

- No new chart *types*. The six existing types are the whole set.
- No change to the `COBOL-CHART-ADD-POINT` / `COBOL-CHART-SET-TABLE` call
  signatures. Their single-series shape stays valid and keeps working.
- No per-series styling properties (per-series colour, dash, axis). Series take
  the existing palette in order.
- No interactive drill-down, selection, zoom or pan. Tooltips are read-only.
- No animation *options* (easing choice, duration property). One built-in curve.

## 3. User stories

- As a COBOL developer binding a table to a chart, I want each numeric sub-field
  drawn as its own series with its own name in the legend, so that one chart can
  compare several measures.
- As a COBOL developer, I want to stack those series, so that a bar chart shows
  both the parts and the total.
- As a COBOL developer with a scatter chart, I want a third sub-field to size the
  points, so that a bubble chart shows a third dimension.
- As a developer feeding a chart from COBOL rather than a binding, I want
  `ValueFields` / `LabelField` / `SeriesLabels` on the control to define the
  series, so that a chart without a binding is not stuck at one.
- As an operator using a running form, I want to hover a bar and read what it is,
  so that I can tell 41 from 43 without squinting at the axis.
- As a developer, I want the chart to animate as it first appears, so that a
  dashboard reads as alive rather than static.

## 4. Requirements (EARS)

### Data model and transport

- **R1 (ubiquitous):** The system shall represent a chart's data as an ordered
  list of **named series**, each carrying an ordered list of values, plus one
  shared ordered list of category labels.
- **R2 (event):** When a chart's data binding resolves, the system shall build
  one series per `ChartValueSeries` mapping in mapping order, take the category
  labels from the `ChartCategory` mapping, and take each series' display name
  from its `ChartSeriesLabel` mapping when one exists and from its `series_id`
  otherwise.
- **R3 (state):** While a chart has **no** resolved data binding, the system
  shall read `ValueFields` as a comma-separated list of sub-field names to draw
  as series, `LabelField` as the sub-field supplying the category labels, and
  `SeriesLabels` as the comma-separated display names for those series in order.
- **R4 (constraint):** The system shall **not** let `ValueFields`,
  `LabelField` or `SeriesLabels` override a resolved binding. The binding is the
  authority; the properties are the fallback. *(Operator decision, 2026-08-18.)*
- **R5 (constraint):** The system shall keep the existing single-series
  `label<TAB>value` form of `__ChartData` valid, and shall draw a chart fed that
  way exactly as it is drawn at 1.61.97.
- **R6 (ubiquitous):** The system shall draw **every** series it is given, with
  no cap. Where there are more series than the palette has distinct colours, the
  palette shall repeat rather than any series being dropped.
  *(Operator decision, 2026-08-18: no caps.)*

### Drawing

- **R7 (optional):** Where `Stacked` is on and a chart carries more than one
  series, a **BarChart** shall stack its series into one bar per category and an
  **AreaChart** shall stack its bands; both shall be drawn grouped/overlaid
  otherwise. Positive contributions shall stack **upward** from zero and negative
  contributions **downward** from zero, so a category holding both shows both.
- **R8 (constraint):** The system shall not stack a Line, Scatter, Pie or Donut
  chart; `Stacked` has no meaning on those and shall be ignored there.
- **R9 (optional):** Where `BubbleField` names a sub-field, a **ScatterChart**
  shall size each point from that field's value, scaled so the largest value in
  the set is drawn at radius `BubbleScale` and the smallest at a visible floor.
- **R10 (state):** While `BubbleField` is empty, a ScatterChart shall size its
  points from `PointRadius`, as at 1.61.97.
- **R11 (event):** When a chart draws a legend and carries several series, the
  legend shall name each series from R2/R3 rather than the placeholder
  `Series N`.
- **R12 (constraint):** A Pie or Donut chart shall draw only the **first**
  series; those types show one series by construction.

### Negative values

*(Added 2026-08-18 by operator decision on Q5: negatives are drawn, not clamped.)*

- **R21 (ubiquitous):** The value axis shall span from `min(0, lowest value)` to
  `max(0, highest value)`, so that zero is always on the axis, and the X axis
  line shall be drawn **at zero** within the plot rather than at the plot floor.
- **R22 (event):** When a value is negative, the system shall draw it on the
  opposite side of the zero line from a positive one — a bar downward, a
  line/area/scatter point below it — and a stacked chart shall stack negatives
  downward and positives upward from zero (R7).
- **R23 (constraint):** While every value in a chart is non-negative, the chart
  shall be drawn exactly as at 1.61.97, with zero at the plot floor. Introducing
  a signed axis shall not move an all-positive chart.

### Interaction

- **R13 (optional):** Where `ShowTooltips` is on, when the pointer rests within
  a data element's own area the chart shall paint a tooltip giving that
  element's category, its series name, and its value.
- **R14 (constraint):** The tooltip shall be painted by the chart itself, not by
  the host UI framework's tooltip facility, so that it works identically on all
  four surfaces and inside the compiled binary.
- **R15 (optional):** Where `AnimateOnLoad` is on, the chart shall animate its
  data elements from zero to their values over a fixed short duration **whenever
  new data arrives** -- including the first arrival -- and shall request repaints
  until the animation completes. *(Operator decision, 2026-08-18: the trigger is
  new data, not the first draw.)*
- **R16 (constraint):** The animation shall be triggered **only** by a change in
  the chart's data. It shall not restart when the chart is re-rendered, hovered,
  scrolled, moved, resized or re-selected, nor when any property other than its
  data changes.
- **R17 (ubiquitous):** Tooltips and the load animation shall act on **every**
  surface a chart is drawn on: the designer canvas, the Preview window, Run Form
  and the compiled binary. *(Operator decision, 2026-08-18.)*
- **R18 (constraint):** The chart shall not require a mutable UI context to
  satisfy R13–R17; it is drawn from a painter, and the clock, the pointer
  position and the repaint request shall be taken from that painter's context.

### Non-regression

- **R19 (constraint):** The system shall not change what a chart draws when none
  of the eight properties is set away from its seeded default, **except** where
  1.61.97 already documented a change.
- **R20 (constraint):** The system shall not alter the `COBOL-CHART-ADD-POINT`
  or `COBOL-CHART-SET-TABLE` call signatures.

## 5. Acceptance criteria

- [ ] **AC1** — A chart bound to a table with two numeric sub-fields draws two
      series, and the legend names them from the binding's `ChartSeriesLabel`
      mappings (falling back to the `series_id`). *(R2, R11)*
- [ ] **AC2** — The same chart with `ValueFields` set to something different
      still draws what the **binding** says. *(R4)*
- [ ] **AC3** — An unbound chart with `ValueFields = "AMOUNT,TAX"`,
      `LabelField = "NAME"` and `SeriesLabels = "Net,Tax"` draws two series named
      `Net` and `Tax` with categories from `NAME`. *(R3)*
- [ ] **AC4** — A chart fed by `COBOL-CHART-ADD-POINT` only draws exactly what it
      draws at 1.61.97; a shape-level comparison shows no difference. *(R5, R19)*
- [ ] **AC5** — A two-series BarChart with `Stacked` on draws one bar per
      category whose height is the sum of the series, and with `Stacked` off
      draws two bars per category. *(R7)*
- [ ] **AC6** — A two-series AreaChart with `Stacked` on draws bands whose upper
      edge is the running total. *(R7)*
- [ ] **AC7** — `Stacked` on a Line, Scatter, Pie or Donut chart changes nothing.
      *(R8)*
- [ ] **AC8** — A ScatterChart with `BubbleField` set draws points whose radii
      differ in proportion to that field, with the largest at `BubbleScale`;
      clearing `BubbleField` returns every point to `PointRadius`. *(R9, R10)*
- [ ] **AC9** — A Pie chart bound to two series draws only the first. *(R12)*
- [ ] **AC10** — With `ShowTooltips` on and the pointer inside a bar, a tooltip
      is painted carrying that bar's category, series name and value; with it off
      nothing is painted. *(R13)*
- [ ] **AC11** — The tooltip is a shape emitted by the chart, verifiable in a
      painted-shape capture with no UI framework tooltip involved. *(R14)*
- [ ] **AC12** — With `AnimateOnLoad` on, a chart whose data has just changed
      renders its elements shorter at t=0 than after the animation duration has
      elapsed; a repaint is requested while it is running. *(R15)*
- [ ] **AC13** — After the animation completes, redrawing the chart — including
      moving, resizing, hovering and re-selecting it — does not restart it;
      pushing **new data** does. *(R16)*
- [ ] **AC14** — AC10 and AC12 hold when the chart is rendered through the
      designer-canvas path as well as the interactive path. *(R17)*
- [ ] **AC15** — A chart given more series than the palette has colours draws
      **all** of them, with the palette repeating; none is dropped. *(R6)*
- [ ] **AC16** — The System KB entries for all eight properties describe the
      behaviour actually delivered, and `assets/knowledge/chunked.data` is
      regenerated in the same change.
- [ ] **AC17** — A BarChart holding a negative value draws that bar **below** the
      zero line and a positive one above it, with the X axis line at zero inside
      the plot. *(R21, R22)*
- [ ] **AC18** — A stacked BarChart whose category holds both a positive and a
      negative contribution stacks the positive upward and the negative downward
      from zero. *(R7, R22)*
- [ ] **AC19** — A chart whose values are all non-negative is drawn identically
      to 1.61.97: zero at the plot floor, shape-for-shape unchanged. *(R23)*

## 6. Constraints & steering check

- **i18n (6 languages):** No new **IDE** strings are expected — the work is in
  `cobolt-forms`, the shared renderer, which is also linked into the compiled
  binary and has no access to the IDE's `Tr` table. Chart-painted text (the type
  badge, the `Series N` placeholder) has always been an English literal there for
  that reason. **If** the work adds a string to an IDE panel, that string is a
  `Tr` field in all six languages. This deviation is noted rather than assumed:
  `/plan` must state, per string, which side of the boundary it falls on.
- **Generated-code / regenerate contract:** Chart series are delivered through
  the data-binding pipeline, which already emits generated COBOL. Any change to
  what codegen emits keeps the `write_header` banner and stays regenerated on
  Build / Run / Debug / Check. No hand-editing.
- **Docs (English guide):** `docs/developers-guide-en.md` gains the multi-series
  chart section and the tooltip/animation/negative-value notes, in the same
  change. The `-es/-pt/-jp/-cn` translations are user-maintained and must not be
  touched.
- **System KB:** all eight property entries in the `cobolt-compiler` docs tables
  are rewritten to describe delivered behaviour, and
  `assets/knowledge/chunked.data` is regenerated and committed in the same
  change.
- **Versioning:** the fix number `z` in `crates/cobolt-ide/src/version.rs` is
  bumped and `CHANGELOG.md` gains an entry. Only the operator raises `x` or `y`.
- **Fix vs feature:** **FIX.** *(Operator decision, 2026-08-18: "these are no
  features as we publish it as complete at some point in the past".)* All eight
  properties were seeded on the control, exposed in the inspector and documented
  in the System KB as working. Delivering them does not add a capability the
  product never claimed — it makes good a claim already published, which is
  exactly the technical-debt category CLAUDE.md rule 4 defines as a fix. It
  therefore lands on a `fix/…` branch and is announced on forum **f=97**, with no
  thread prefix — **not** f=96 and not `[Noticia]`.
- **Paint baseline:** `paint::elegance_baseline_tests` pins an exact painted-shape
  count for a 27-control fixture. Any deliberate change to what a chart draws
  must re-bless it **with the arithmetic accounted for**, as 1.61.97 did.

## 7. Open questions

All five resolved by the operator on 2026-08-18; recorded here so the reasoning
survives.

| | Question | Answer |
|---|---|---|
| **Q1** | How many series? | **No cap.** Draw every series given; the palette repeats. → R6, AC15 |
| **Q2** | What triggers the animation? | **New data arriving**, not the first draw. Editing the chart never replays it. → R15, R16, AC12, AC13 |
| **Q3** | Tooltip while designing? | **Yes**, suppressed while a pointer button is down so it does not fight the designer's drag/select. → R13 |
| **Q4** | `BubbleField` without a binding? | **Binding only** — with no binding there is no sub-field to read. The KB says so. → R9 |
| **Q5** | Stacking negatives? | **Do not clamp.** Draw negatives **below the X axis**; the axis sits at zero. → R21, R22, R23, AC17–AC19 |

No open questions remain. Ready for `/tasks` review and `/implement`.
