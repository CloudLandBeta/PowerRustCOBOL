# Spec — 3D perspective for display controls: `RotateX`, `RotateY` and `PerspectiveDistance`

- **Status:** implemented in 1.91.39 — scope, property shape and distance meaning decided by the operator on 2026-10-13 (§8); the open items of §7 were taken at their assumed answers
- **Folder:** specs/096-control-3d-perspective/
- **Author:** operator request, written by Claude   **Date:** 2026-10-13
- **Branch:** `features-perspective-3d` (a per-change branch off `main`: `features` is held by another worktree, `specs/steering/structure.md`)

## 1. Overview
The operator asked for properties that turn a control in space "like the simulation": a
**Revenue by quarter** bar chart drawn as a card tilted away from the viewer, its left edge
farther than its right, over a dark photographic backdrop; and a second picture, a line
sketch of the same card as a slab in perspective. The properties wanted are **which way it
tilts — top, bottom, left or right — and how far or close the control is from the
observer**.

This spec adds three properties to the controls that only *draw*:

| Property | Meaning | Values | Default |
|---|---|---|---|
| `RotateX` | tilt about the horizontal axis — the **top** or **bottom** edge recedes | whole degrees, −60 … 60 | 0 |
| `RotateY` | tilt about the vertical axis — the **left** or **right** edge recedes | whole degrees, −60 … 60 | 0 |
| `PerspectiveDistance` | how far or close the control is from the observer | whole number, 0 … 100 | 50 |

Facts the requirements rest on, each read in the source on 2026-10-13, none recalled:

1. **Nothing in the engine rotates, skews or projects.** No `Rotat…`, `Tilt`, `Skew` or
   `Perspective` property name appears in `cobolt-forms`, `-ide`, `-compiler`, `-runtime`,
   `-form-host` or `-codegen`, and the only `TSTransform` in `cobolt-forms` is a translation
   (`paint.rs`, three uses, all `from_translation`). A control is drawn at an axis-aligned
   rectangle.
2. **One painter draws a control's face for every surface.**
   `paint::draw_control(painter, origin, ctrl, selected, glass, alpha_mul, scale, pic_tex)`
   is what the designer canvas, Preview, Run Form and a compiled binary all reach (spec 017,
   `render_faces` and `render_form`). A chart is painted by it as shapes; a PictureBox and an
   Animator add a textured image to the same painter.
3. **A control's properties are whatever `Control::new` seeds for its type.**
   `property_names_for` is built from that seed, so a new property is declared there, per type,
   and is then settable and gettable from COBOL by name, stored in the `.cfrm`, and offered
   by IntelliSense. The Properties pane is separate code per type (`int_prop_row(…, range, …)`).
   `THEME_OWNED_PROPS` lists what a theme switch may rewrite; the new properties are the
   developer's and are not in it.
4. **A display control already has pointer events, with no coordinates.** The 7 charts, Line
   and Shape declare `onClick`, `onDblClick`, `onMouseEnter`, `onMouseMove`, `onHoverEnter`…;
   `control_pointer_events` decides them from a **rectangle**, and no event carries a mouse
   position.
5. **A Label is not purely painted at run time.** Its caption is returned by
   `draw_control_capturing_label` and hosted through egui's selectable-label machinery, and a
   Label is a Tab target (`takes_tab_order`).
6. **The loader keeps every property it meets.** `read_form` inserts each `<Property name=…>`
   into the control's map whatever the type (`xml.rs`, `PropertyStart`), so "does not have the
   property" can only mean *not offered and not read*, never *not stored*.
7. **egui cannot project a widget.** A TextBox, a ComboBox or a Slider is an egui widget
   that reads and writes the pointer in screen space; nothing in this stack draws one
   off-screen first. That is why the operator limited this spec to controls that only draw.

## 2. Goals / Non-goals
- **Goals:**
  - Three properties on the 12 display control types of §4 R1, with the neutral values
    above, settable in the Designer, in the `.cfrm` and from COBOL.
  - The projected control looks the same on every surface that draws it — designer canvas,
    Preview, Run Form, compiled binary — because it is one projection in the one engine.
  - A control left at the defaults costs nothing and draws exactly what it drew before.
  - A pointer event on a tilted control happens where the control **is**, not where its
    designed rectangle was.
- **Non-goals:**
  - A control that takes input (Button, TextBox, ComboBox, CheckBox, ListBox, Slider,
    DataGrid, NumericUpDown, DateTimePicker, TreeView, Knob, Switch, Maps …), and a container
    with children (Panel, GroupBox, TabControl, Splitter): none gets the properties.
  - Rotation about the viewing axis (a roll), a tilt about a point other than the centre,
    skew, mirroring.
  - An extruded thickness or side face (the slab of the operator's line sketch), lighting,
    shading or reflections.
  - Depth ordering between two tilted controls that intersect in space: controls keep the
    paint order they have (z-order, layers); no z-buffer.
  - Animating the properties by a built-in effect. A COBOL Timer can step them; an
    `AnimationDef` for them is a later spec.
  - The form window itself in perspective.

## 3. User stories
- As a **dashboard designer**, I want to set a chart's `RotateY` to −30 in the Properties
  pane and see it turn on the canvas, so that I can lay out a dashboard of cards in
  perspective without a pre-rendered picture.
- As a **COBOL programmer**, I want `MOVE 30 TO CHART-1::RotateY` and
  `MOVE 75 TO CHART-1::PerspectiveDistance`, so that a handler can swing or bring forward a
  chart the way it changes any other property.
- As a **form designer**, I want to still select, move and resize a tilted control by its
  designed rectangle, so that tilting does not make it impossible to edit.
- As an **application user**, I want a click on a tilted picture to count only where I see
  the picture.

## 4. Requirements (EARS)
- **R1 (ubiquitous):** The system shall give the properties `RotateX`, `RotateY` and
  `PerspectiveDistance` to exactly these control types: `BarChart`, `LineChart`, `PieChart`,
  `AreaChart`, `ScatterChart`, `DonutChart`, `RadarChart`, `PictureBox`, `Animator`, `Label`,
  `Line` and `Shape`.
- **R2 (ubiquitous):** `RotateX` and `RotateY` shall each be a whole number of degrees from
  −60 to 60, default 0; `PerspectiveDistance` a whole number from 0 to 100, default 50.
- **R3 (ubiquitous):** The edge that moves **away** from the observer shall be:

  | Value | Edge that recedes |
  |---|---|
  | `RotateX` > 0 | top |
  | `RotateX` < 0 | bottom |
  | `RotateY` > 0 | right |
  | `RotateY` < 0 | left |

  An edge that recedes is the shorter on screen; the opposite edge is the longer. The
  operator's reference (left edge farther than the right) is a negative `RotateY`.
- **R4 (ubiquitous):** The projection shall be anchored at the **centre of the control's
  designed rectangle**: the centre stays where it is, and a form's layout is not changed by
  a tilt.
- **R5 (ubiquitous):** `PerspectiveDistance` shall act on **both** the strength of the
  perspective and the apparent size, and shall be monotonic: for any fixed tilt, a larger
  value gives a **larger** projected control and **stronger** convergence (a larger ratio of
  the nearer edge to the farther); 50 is the reference distance, at which a control that is
  not tilted is drawn at its designed size; 0 is the farthest and 100 the nearest.
- **R6 (state):** While `RotateX` = 0 and `RotateY` = 0 and `PerspectiveDistance` = 50, the
  control shall be drawn **exactly as before this feature** — the same shapes, no extra
  work.
- **R7 (ubiquitous):** The projection shall apply to everything the control draws as one
  piece — face, fill and gradient, border and rounded corners, shadow, caption and chart
  text, chart marks, and the image of a PictureBox or an Animator — so that straight edges
  stay straight, text stays whole and legible, and an image or a gradient is not smeared
  across the perspective.
- **R8 (ubiquitous):** The projected control shall be drawn by the same engine on every
  surface (designer canvas, Preview, Run Form, compiled binary), with the same geometry.
- **R9 (ubiquitous):** The projected control shall paint at its own place in the paint order
  (z-order and layer, specs 017 and 091) and shall be clipped by the same rectangle that clips
  an untilted control in its place (its container's content rectangle; the form's rectangle
  for a control on a layer). Nothing of it shall be drawn outside that rectangle.
- **R10 (event):** When the pointer is moved or pressed, the pointer events of a tilted
  control (`onClick`, `onDblClick`, `onRightClick`, `onMouseDown`, `onMouseUp`,
  `onMouseMove`, `onMouseEnter`, `onMouseLeave`, `onHoverEnter`, `onHoverLeave`,
  `onContextMenu`, `onMouseWheel`) shall arise only for a pointer **inside the projected
  outline**, and for every such position, including one outside the designed rectangle.
- **R11 (state):** While the Form Designer has a tilted control selected, the selection
  rectangle, its handles, moving and resizing shall stay on the **designed rectangle**,
  exactly as for any control; the projected shape is drawn live on the canvas.
- **R12 (event):** When COBOL writes `RotateX`, `RotateY` or `PerspectiveDistance` of a
  control, the control shall be drawn with the new value from the next frame, in a running
  form (Run Form and a compiled binary alike); a read shall answer the current value. A value
  outside the range shall be **clamped** into it, never an error.
- **R13 (ubiquitous):** The properties shall be saved in the `.cfrm` like any other and a
  form saved before this feature shall load with the neutral values and look as it did.
- **R14 (state):** While a Label is tilted, its caption shall be painted with the projection
  and shall **not** be selectable as text; the Label shall keep its pointer events (R10), its
  tab stop and its `onGotFocus`.
- **R15 (state):** While an Animator or a PictureBox with animated frames is tilted, it shall
  keep playing.
- **R16 (ubiquitous):** A control of any other type shall not offer the properties: the
  Properties pane does not show them and IntelliSense does not list them; and a control of
  another type that carries them anyway (a `.cfrm` edited by hand — the loader keeps every
  `<Property>` it reads, whatever the type) shall **ignore** them: it is drawn untilted and
  loads without an error.
- **R17 (ubiquitous):** The Properties pane shall show the three properties for the 12 types,
  with a help line in all six IDE languages; the Developer's Guide shall describe them (with
  the reference picture); the System KB tables shall document them and the pre-embedded store
  shall be regenerated in the same change.
- **R18 (constraint):** The system shall not change the rectangle a control occupies in the
  layout, its tab order, its events' bindings or its COBOL identifiers, and shall not
  rewrite the three properties on a theme switch.
- **R19 (ubiquitous):** The cost of a tilted control shall be **measured and reported**
  (a bench, `cobolt-bench`), and a form with no tilted control shall show no measurable
  change.

## 5. Acceptance criteria
- [ ] **AC1 (R1, R2, R13, R16)** — for each of the 12 types, the three properties exist with
  the defaults of R2, survive a `.cfrm` save and load, and load as neutral from a form that
  never had them; for the control types of the non-goals, `property_names_for` does not
  list them, and a Button whose `.cfrm` names `RotateY` loads, is drawn exactly as a Button
  without it, and is not tilted (a test per type, in the manner of `tests/controls/`; the
  count of types is asserted, so a thirteenth type cannot slip in).
- [ ] **AC2 (R6)** — at the defaults, a control's shapes are identical to the ones before the
  change: the 75-form corpus goldens of `cobolt-forms` and `cobolt-form-host` pass **without
  being regenerated**, and a unit test compares the shapes of each of the 12 types, neutral,
  with the shapes of the previous code path.
- [ ] **AC3 (R3)** — the projected outline of a rectangle: `RotateY` = +30 → its right edge is
  shorter than its left; −30 → the reverse; `RotateX` = +30 → its top edge is shorter than its
  bottom; −30 → the reverse; both → the corner nearest the observer is the largest (a pure
  geometry test of the projection, printing the four edge lengths).
- [ ] **AC4 (R4, R5)** — at `RotateX` = `RotateY` = 0: distance 50 gives the designed
  rectangle, 0 a strictly smaller and 100 a strictly larger one, each about the same centre;
  at a fixed tilt of 30°, the ratio nearer-edge/farther-edge and the projected size increase
  strictly with the distance over 0, 25, 50, 75, 100 (the five values printed).
- [ ] **AC5 (R7)** — in the shapes of a tilted chart, a tilted PictureBox and a tilted
  Label, every straight edge lies on a straight line of the projected outline within half a
  pixel; no text glyph is degenerate (no NaN, no zero-area quad); an image's four corners
  land on the projected outline's four corners, and a point halfway along an edge of the
  image lands halfway in perspective, not halfway on screen (the test that a texture is
  mapped perspective-correctly, not affinely).
- [ ] **AC6 (R8)** — the same tilted control produces the same projected geometry from
  `render_faces` (designer) and `render_form` (Preview, Run), by extending the existing parity
  guard `engine_reference_form_parity_static_vs_faces` with a tilted chart, picture and shape.
- [ ] **AC7 (R9)** — a tilted, enlarged chart inside a Panel paints nothing outside the
  Panel's content rectangle, and one on a layer nothing outside the form's rectangle (a test
  of the painted shapes' bounds); a control drawn later in z-order is over it.
- [ ] **AC8 (R10)** — for a tilted Shape: a pointer inside the projected outline and inside
  the designed rectangle → `onClick`; inside the outline and **outside** the designed
  rectangle (an enlarged one) → `onClick`; inside the designed rectangle and **outside** the
  outline → none; a table of the pointer positions tried and the answers is printed.
- [ ] **AC9 (R11)** — in the designer, the selection rectangle and handles of a tilted
  control are the designed rectangle's, and a drag from a handle resizes the designed
  rectangle.
- [ ] **AC10 (R12)** — a form run through the form host with a handler that moves
  `CHART-1::RotateY` from 0 to 30 draws the chart with the new outline on the next frame; an
  out-of-range write (`MOVE 200 TO …::RotateX`) reads back 60; the same run through the
  compiled-binary host (the three hosts that build the interpreter, skill
  `interpreter-binary-parity`) behaves the same.
- [ ] **AC11 (R14, R15)** — a tilted Label draws the caption with the projection and offers no
  text selection, and still raises `onClick` where it is seen; a tilted Animator advances
  frames between two frames of the test clock.
- [ ] **AC12 (R17)** — a picture of the reference scene (a *Revenue by quarter* chart,
  `RotateY` −30, `RotateX` 8, `PerspectiveDistance` 60, over a dark backdrop), made by
  `render_form`, is in the Guide and has been **looked at** by the author of the change; the
  Properties pane shows the three properties for each of the 12 types (a drawing test, in
  six languages, with none of their words left in English); IntelliSense lists them after
  `CHART-1::`; the System KB documents them and `prebuilt_chunked_kb_matches_the_published_
  documentation` is green.
- [ ] **AC13 (R18)** — the layout rectangle, the tab order and the event bindings of a control
  are equal before and after setting the properties; a theme switch leaves the three
  properties as they were.
- [ ] **AC14 (R19)** — a bench reports, for a form of 1 and of 8 tilted charts, the time of
  one frame next to the same form untilted; the numbers are in the CHANGELOG as measured.

## 6. Constraints & steering check
- **i18n ×6:** yes. The Properties pane's help for each property and its section name are
  texts in the Properties-pane table (`ui_text`), in all six languages; the property names
  themselves stay English, like every property. No hard-coded UI literal.
- **Generated-code / regenerate contract:** no impact. `cobolt-codegen` does not read a
  control's properties (a search of its sources for `.properties`, `CornerRadius`,
  `ShadowEnabled` and `Transparency` finds nothing): the form host reads them from the
  `.cfrm`. The developer banner and regenerate-on-Build/Run/Debug/Check are untouched.
- **Docs:** the **English** Developer's Guide (§ on painting and layout, and the property
  tables); translations are not touched (the regeneration cycle of GOLDEN RULE #8: deleting
  them is the rule's way, and it keeps `docs_embed` red until the next minor).
- **System KB:** yes — the `cobolt-compiler` property tables and
  `cargo run -p cobolt-ide --example build_chunked_kb` in the same change.
- **Render engine:** one painter, one projection (spec 017); the three-hosts rule applies
  to the run-time write of R12.
- **Rounded corners:** a projected control's rounded corners are a projection of the arc,
  not a repair of it: no notch mask on a tilted control (read `rounded-corners` before
  touching the corner code in `/plan`).
- **Fix vs feature:** **feature** — a capability beyond the COBOL-85 standard and the IDE's
  existing scope (a new property family on a control). Branch `features-perspective-3d`;
  version `z` bump and a CHANGELOG entry; announced, if ever, on f=96 (GOLDEN RULE #4b).

## 7. Open questions
Each has the answer this spec assumes if nothing is said.
- **Q1 — Label.** It is in the set the operator named, but it is the one type with a hosted,
  selectable caption and a tab stop (§1.5). *Assumed:* in, with the limits of R14. Dropping it
  from this version is a one-line change to R1.
- **Q2 — Which controls beyond the 12.** `ProgressBar` and `Gauge` only draw too, and were
  not named. *Assumed:* not in this version.
- **Q3 — Sign convention.** R3 makes a positive `RotateX` recede the top and a positive
  `RotateY` the right edge. *Assumed:* as written; the reference picture is `RotateY` < 0.
- **Q4 — Whole degrees.** *Assumed:* whole degrees (like `CornerRadius` and `Transparency`);
  a Timer stepping one degree at a time is smooth enough. A fractional value is a change to
  R2 and to the property's type.
- **Q5 — The slab.** The line sketch draws the card with a thickness. *Assumed:* a non-goal
  (§2); the reference picture, the chart, has none.
- **Q6 — Animation.** *Assumed:* out of scope (§2).

## 8. Decisions of the operator (2026-10-13)
1. **Which controls:** display controls only — not a control that takes input, not a
   container with children.
2. **The properties:** two rotation angles and a distance (`RotateX`, `RotateY`,
   `PerspectiveDistance`), not a direction name plus an angle.
3. **What distance does:** both the strength of the perspective and the apparent size
   (closer = stronger and larger; farther = flatter and smaller).
4. **How far to go now:** spec, plan and tasks, then stop for review.
