# Grid-aligned placement

The geometry you send is put on the form's designer grid before it is applied.
The form owns the grid: `GridSize` is the cell in pixels (8 by default) and
`SnapToGrid` turns it on or off.

The grid is applied ONCE per axis, not once per control, in two steps.

**Coordinates within half a cell of each other are one position.** The first
control to use a coordinate on an axis opens the lane; anything that close is
that same lane. A column at `X=19`, `X=21`, `X=20` comes out as one column, not
three positions a cell apart.

**The whole run is then translated, not quantised.** The shift that puts the
FIRST lane exactly on the grid is applied to every lane on that axis. So every
distance you asked for is kept to the pixel — a 30px row pitch stays 30px, a
180px column gap stays 180px — and only the first placement lands on a grid
point. The rest sit exactly where your own spacing puts them.

What this means for the coordinates you choose:

- **Spacing is yours and it is honoured exactly.** Pick the row pitch and column
  gap you actually want; nothing will round them into a lumpy 24/32/32 rhythm.
- **The first control you place anchors both axes.** Put it on a grid multiple
  (8, 16, 24, 160, 320) and the whole layout lands on the grid with it. Anchor
  on 19 and the entire run shifts by -3.
- **Give every control in a column the SAME `X`, and every control in a row the
  SAME `Y`.** Identical coordinates are what makes an alignment unambiguous;
  near-misses rely on the half-cell catchment and read as accidents.
- **A deliberate second column belongs a whole cell away or more.** Anything
  closer than half a cell is treated as the same column and pulled into line
  with it.
- **When `SnapToGrid` is off nothing is moved at all** — your coordinates are
  used verbatim — so alignment is entirely yours to get right.
