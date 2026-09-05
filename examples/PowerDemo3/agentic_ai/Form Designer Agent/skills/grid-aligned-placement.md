# Grid-aligned placement

Every `X`/`Y` you send lands on the form's designer grid, exactly as a control
dragged by hand does. The form owns the grid: `GridSize` is the cell in pixels
(8 by default) and `SnapToGrid` turns it on or off. A coordinate off the grid is
moved to the **nearest** grid point — `X=19` becomes `16`, `X=21` becomes `24`.

Snapping alone would break the alignments you asked for, so it does not run
coordinate by coordinate. The **first** control to use a coordinate on an axis is
snapped, and every later control within half a cell of it is given that same
value. A column of checkboxes at `X=19`, `X=21`, `X=20` comes out as one column,
not three positions a cell apart.

Treat this as a safety net, not a substitute for deliberate layout:

- **Place controls on grid multiples yourself.** With the default 8px grid use
  8, 16, 24, 160, 320 — not 19, 150, 370. Then what you compute is what the
  developer sees, and nothing has to be corrected on the way in.
- **Give every control in a column the SAME `X`, and every control in a row the
  SAME `Y`.** Identical coordinates are what makes an alignment unambiguous;
  near-misses rely on the half-cell catchment and read as accidents.
- **A deliberate second column belongs a whole cell away or more.** Anything
  closer than half a cell is treated as the same column and will be pulled into
  line with it.
- **Row pitch is snapped per row, so pick a pitch that is a multiple of the
  cell.** A 30px step on an 8px grid lands 32, 24, 32, 24…; a 32px step stays
  even.
- **When `SnapToGrid` is off nothing is moved** — your coordinates are used
  verbatim — so alignment is entirely yours to get right.
