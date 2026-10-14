# Spec — the corner pin: fit a tilted control to any drawn panel

- **Status:** implemented in 1.91.145 (the review gates of /specify, /plan and /tasks are taken together: the operator chose the corner pin over `RotateZ` from the 2026-10-14 handoff, which already holds the analysis and the design)
- **Folder:** specs/098-perspective-corner-pin/
- **Author:** operator request, written by Claude   **Date:** 2026-10-14
- **Branch:** `features-perspective-3d` (continues specs 096 and 097)

## 1. Overview
With `RotateX`, `RotateY`, `PerspectiveDistance` and `PerspectiveStrength` a display control is a
tilted rectangle seen head-on: its projected outline is always symmetric about a level mid-line
through its centre. The operator laid a tilted PieChart over the glass panel of a background picture
and drew the panel's edges and the chart's: the panel is **rolled** about 2.5° in the picture and
converges differently from the chart; no value of the four properties matches it
(`HANDOFF-2026-10-14-perspective-fit.md`, *The open problem*). A drawn panel is any convex
quadrilateral — eight numbers; a tilt is three.

This spec adds a **corner pin**: eight properties that move each corner of the projected outline by
a whole number of pixels, and in the Form Designer a handle at each corner of the outline that the
developer drags onto the panel's corner. The control is then projected onto exactly that quadrilateral.

| Property | Meaning | Values | Default |
|---|---|---|---|
| `PinTopLeftX`, `PinTopLeftY` | how far the top-left corner of the outline is moved | whole pixels, −9999 … 9999 | 0 |
| `PinTopRightX`, `PinTopRightY` | … the top-right corner | same | 0 |
| `PinBottomRightX`, `PinBottomRightY` | … the bottom-right corner | same | 0 |
| `PinBottomLeftX`, `PinBottomLeftY` | … the bottom-left corner | same | 0 |

The offsets are added to the corners the tilt produces, so a pin works **with** a tilt (nudge a tilted
chart's corners onto the panel) and **without** one (pin a flat chart to any quadrilateral).

Facts the requirements rest on, read in the source on 2026-10-14:

1. The projection of specs 096/097 is a pin-hole camera, `screen = c + (x₂, y₁)·F/(D + m·z₂)`
   (`perspective.rs`, `Camera::project`). The pin-hole image of a plane is a **homography**, and a
   homography is fixed by the images of four points — so the outline's four corners determine the
   map of every point, and any convex quadrilateral is reachable.
2. Every consumer of the projection — the mesh warp (`warp_mesh`), the clip rule (`clip_case`), the
   hit test (`quad_contains` on `hit_quads`), the three render hooks, the Designer's knobs — asks only
   `Camera::project` and `Perspective::quad`. Lines stay straight under a homography, which is all
   `warp_mesh`'s subdivision relies on.
3. The Designer already keeps **two kinds of handle**: the eight knobs on a control's rectangle and a
   Line's two endpoint handles (`line_endpoints_of`, `DragState::ResizingLineEndpoint`), captured at
   press time (`press_line_anchor`) and written live during the drag with one undo step at the end.
4. A property seeded in `Control::new` is saved, settable from COBOL (clamped in
   `property_runtime_text` for the three hosts), listed by IntelliSense, and round-tripped
   (`tests/controls` generic test).

## 2. Goals / Non-goals
- **Goals:** any convex quadrilateral as the outline of a display control, by eight properties and by
  dragging four handles on the canvas; a control without offsets drawn **exactly** as before.
- **Non-goals:** a non-convex (folded) outline — refused, never drawn; pinning a control that takes
  input or holds children; a curved edge; a separate roll property (`RotateZ`: the pin covers it).

## 3. User stories
- As a **dashboard designer**, I want to drag the four corners of a tilted chart onto the corners of the
  panel in my background picture, so that the chart sits in the picture's own perspective.
- As a **COBOL programmer**, I want `MOVE 24 TO CHART-1::PinTopLeftY`, so that a handler can move a
  corner like any other property.

## 4. Requirements (EARS)
- **R1 (ubiquitous):** The system shall give the eight pin properties to exactly the 12 control types of
  spec 096 R1, whole numbers −9999 … 9999, default 0.
- **R2 (ubiquitous):** The projected outline shall be the outline the tilt properties give, **each corner
  moved by its pin**; every point of the control shall be mapped by the one homography that sends the
  designed rectangle's corners onto that outline, so that straight lines stay straight and the corners
  of a picture land on the corners of the outline.
- **R3 (state):** While all eight pins are 0 or absent, a control shall be drawn **exactly** as it was
  before this spec (the pin-hole formula, not a homography fitted to it), and a form saved before shall
  load with them absent.
- **R4 (ubiquitous):** The pins shall be in the control's **designed** pixels and shall scale with the
  control when an animation or the surface scales it, so that a pinned control zooms in as one shape.
- **R5 (ubiquitous):** A pin that would make the outline **non-convex, or reverse its winding** shall not
  be drawn: the control shall be drawn with its tilt alone (as if the eight pins were 0) and the
  Designer shall refuse to drag a handle to such a place. Nothing shall ever be behind the camera.
- **R6 (state):** While the Form Designer has a display control selected that is tilted, zoomed or pinned,
  it shall show a **handle at each corner of the projected outline**, distinct from the eight resize
  knobs, which stay on the designed rectangle (spec 096 R11); dragging a corner handle shall move that
  corner, write its two pins live, and record **one** undo step for the gesture.
- **R7 (event):** When COBOL writes a pin outside its range it shall be clamped and read back clamped, in all
  three hosts.
- **R8 (ubiquitous):** A pinned control's pointer events shall arise inside the pinned outline (spec 096
  R10); the Properties pane shall show the eight properties for the 12 types with help in six languages;
  IntelliSense, the System KB and the Developer's Guide (with a picture) shall carry them.

## 5. Acceptance criteria
- [x] **AC1 (R1, R3, R7)** — the 12 types seed the eight pins at 0 and no other type does; a form saved without
  them loads without them and re-saves byte for byte; `MOVE 20000` reads back 9999 in the three hosts.
- [x] **AC2 (R2)** — a pinned control's four outline corners are the tilt's corners plus the pins to 1e-3 px;
  for a tilted chart pinned to a chosen quadrilateral every projected vertex lies where the homography
  puts it; a picture's four corners land on the quad's four corners and its mid-edge texel in perspective.
- [x] **AC3 (R3)** — at eight zero pins the projection equals the spec 097 formula at every point tried, to
  0 px; the corpus goldens pass unregenerated.
- [x] **AC4 (R4)** — a pinned control under an animation scale of 0.5 and 1.5 has its pins scaled with it.
- [x] **AC5 (R5)** — a pin that folds the outline leaves the control drawn with its tilt alone; the homography's
  denominator stays positive over the whole rectangle for a sweep of random convex quadrilaterals.
- [x] **AC6 (R6)** — in the Designer a tilted chart shows four corner handles at the outline's corners and the
  resize knobs at the rectangle's; dragging a corner handle moves that corner by the drag, writes its two
  pins, is one undo step, and a drag that would fold the outline is refused.
- [x] **AC7 (R8)** — a click inside the pinned outline but outside the tilt's outline reaches the control;
  the pane shows the rows for the 12 types in six languages; IntelliSense lists them; the KB guard is
  green; the Guide picture has been looked at.

## 6. Constraints & steering check
- **i18n ×6:** the eight row labels (`ui_text_data.rs`) and help texts (`prop_help_data.rs`).
- **Generated code:** untouched. **Docs:** the English Guide. **System KB:** yes, regenerated.
- **Fix vs feature:** **feature**; `z` bump; CHANGELOG.

## 7. Open questions (each taken at the answer assumed here)
- **Q1 — Handles on a flat, unpinned control?** They would sit exactly on the corner knobs. *Assumed:* the corner
  handles appear once the control is tilted, zoomed or pinned (`Perspective::of` is `Some`); to pin a flat
  control, type one pin in the pane first. Revisit if the operator finds it awkward.
- **Q2 — Range.** ±9999 pixels: wider than any form. *Assumed.*
- **Q3 — Name.** `Pin…` rather than `Corner…`, to stay clear of `CornerRadius` / `CornerRadiusTopLeft`. *Assumed.*
