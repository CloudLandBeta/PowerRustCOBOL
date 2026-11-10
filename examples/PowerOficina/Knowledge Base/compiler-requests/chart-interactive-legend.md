# Gap report: interactive legend on charts (click isolates, click again undoes)

Date: 2026-11-08. Raised by: PowerOficina (Dashboard). Status: open.

## What the application needs
A chart legend whose items can be clicked:
- a click on an item shows only that series (bar, line, area, radar) or slice (pie, donut);
- clicking several items shows all of them; a new click on a marked item unmarks it;
- with no item marked, every series or slice is shown again;
- each series keeps its colour while others are hidden, the legend shows each item's state
  (marked or not), and the data, totals and bound values are never changed by it;
- works on the desktop painter and in the browser player (class B charts, SVG), with the
  same property on both.

## What the reference offers today
`BarChart`, `LineChart`, `AreaChart`, `PieChart`, `DonutChart`, `RadarChart` have `ShowLegend`,
`SeriesLabels`, `SeriesColors`, `ShowTooltips`, `AnimateOnLoad`, `AnimateValues`. None of them
reacts to a click on the legend, and no method or event exposes legend items
(`kb_lookup` / `kb_search` on legend, toggle, hide, series visibility found nothing).

## Workaround in use
`ShowLegend` is 0 and the form draws its own legend with one Button per item
(`BTN-LEG-M1/M2/P1/P2/P3` in `webforms/main-form.wcfrm`). A click flips a flag, then the form
clears the chart, resets `SeriesColors`/`SeriesLabels` and adds only the marked points.
It costs a button row per chart, hand-kept colours that must match the chart's, and
repeated logic in every form that wants it.

## Proposed shape (for the maintainers to adjust)
- Property `LegendInteractive` (Boolean, default 0): legend items are clickable.
- Runtime property `HiddenSeries` (read/write, comma-separated names or 1-based indexes) so a
  program can read or set the state.
- Event `onLegendClick` with the item name and its new state.
- The painter (desktop) and the web player (`view/chart_paint.rs`, `view/charts.rs`) draw
  hidden items as dimmed legend entries and leave their marks out; scales follow the
  visible data only when `AutoScaleVisible` is on (default 1).

## Acceptance
Clicking an item of a two-series BarChart and a three-slice DonutChart behaves as described
above in `render_form`/`run_form` and in the browser, with `AnimateValues` moving the marks.
