# Spec — Zoom in the Form Designer

- **Status:** draft
- **Folder:** specs/089-designer-zoom/
- **Author:** operator request, written by Claude   **Date:** 2026-10-05

## 1. Overview
The Form Designer paints a form at exactly its own size: one form pixel is one
screen point. Small controls, controls packed close together (PowerSpatial's
cards hold 9 px labels a few pixels apart), and the grips and handles around
them are then hard to hit and hard to read. This feature adds a **designer-only
zoom**. The canvas is magnified or reduced for editing, while the form itself
does not change. Its size, and what Preview, Run Form and a built application
show, stay exactly as designed.

## 2. Goals / Non-goals
- **Goals**
  - Zoom the designer canvas in and out, quickly, from the toolbar, the mouse
    wheel and the keyboard.
  - Every canvas gesture (select, move, resize, place a new control, rubber
    band, grid editing from spec 088) works at any zoom and writes the same
    form values it would write at 100 %.
  - Zoomed text stays sharp, painted at the zoomed size rather than scaled up
    as a bitmap.
- **Non-goals**
  - No change to the form's size, its controls' geometry, `FontScale`, the
    breakpoint "View at" size (spec 056 R65), or anything saved in the `.cfrm`.
  - Preview, Run Form and built applications are not zoomed. They are not
    touched at all.
  - No zoom in the code editor or other panels (the Documentation viewer
    already has its own).

## 3. User stories
- As a form designer, I want to zoom in on a crowded card so that I can grab
  the right label and its handles without hitting its neighbour.
- As a form designer, I want to zoom out to see a large form whole while I
  arrange its sections.
- As a form designer, I want to return to 100 % with one keystroke, so that I
  can judge the real size.

## 4. Requirements (EARS)

### 4.1 Controls
- **R1 (ubiquitous):** The Form Designer's icon toolbar shall carry icon-only
  **Zoom out**, **Zoom in** and **Fit** buttons, with translated tooltips, and
  a **zoom readout** (e.g. `150 %`) that, when clicked, returns to 100 %.
- **R2 (ubiquitous):** Zoom shall step through 25, 33, 50, 67, 75, 90, 100,
  110, 125, 150, 175, 200, 250, 300 and 400 %. 25 % and 400 % are the limits,
  and the Zoom out / Zoom in buttons are disabled at them.
- **R3 (event):** When the developer turns the mouse wheel (or pinches on a
  trackpad) over the canvas with **Ctrl** (Windows/Linux) or **Cmd** (macOS)
  held, the zoom shall change by one step, keeping the form point under the
  pointer fixed on screen.
- **R4 (event):** When the canvas has keyboard focus, **Ctrl/Cmd + `+`**,
  **Ctrl/Cmd + `−`** and **Ctrl/Cmd + `0`** shall zoom in, zoom out and return
  to 100 % respectively. Toolbar-triggered zoom keeps the canvas centre fixed.
- **R5 (event):** When **Fit** is pressed, the zoom shall become the largest
  step at which the whole form fits in the visible canvas area.

### 4.2 Painting
- **R6 (state):** While the zoom is not 100 %, the canvas shall paint the form,
  every control, its text, images, borders, shadows and corner rounding scaled
  by the zoom factor. Text shall be laid out at the scaled font size so it
  stays sharp.
- **R7 (ubiquitous):** Designer overlays (selection handles, grips, the
  alignment guides, the snap grid dots, the grid-track lines and labels of
  056 R45 / spec 088, the floating grid toolbar, drop markers) shall keep their
  **on-screen** size at every zoom. Only their positions follow the zoom.
- **R8 (state):** While the zoomed form is larger than the visible canvas
  area, the canvas shall scroll on both axes (scroll bars, plain wheel
  vertically, Shift+wheel horizontally). While it is smaller, it shall sit
  where it sits today.
- **R9 (ubiquitous):** At 100 % the canvas shall paint exactly as it does
  today. The existing static-vs-faces parity guard
  (`engine_reference_form_parity_static_vs_faces`) stays green unchanged.

### 4.3 Editing at any zoom
- **R10 (ubiquitous):** Every canvas gesture shall convert pointer positions
  from screen to form coordinates through the zoom, so that the values it
  commits (x, y, width, height, grid tracks, placements) are the ones the same
  gesture would commit at 100 %.
- **R11 (ubiquitous):** Snap-to-grid, the drag threshold and the minimum sizes
  shall be measured in **form** pixels, except the drag threshold, which is
  measured on screen so a click is a click at any zoom.
- **R12 (constraint):** Zoom shall never change the form: zooming alone shall
  not mark it dirty, add an undo step or alter any byte of the saved `.cfrm`.

### 4.4 Scope and memory
- **R13 (ubiquitous):** Zoom is per designer window. Each open form has its
  own zoom, and every form opens at 100 %.
- **R14 (constraint):** The form's own window grip (resizing the form in the
  designer) shall still change the form's **design** size, converted through
  the zoom. The designer window itself shall never resize because of zoom
  (GOLDEN RULE: a window never resizes itself).
- **R15 (ubiquitous):** The "View at" breakpoint bar shall keep showing the
  form's design size (e.g. `1000 × 600`), never the zoomed size.

## 5. Acceptance criteria
- [ ] **AC1 (R1, R2)** — The toolbar shows Zoom out, readout, Zoom in and Fit,
  with tooltips in all six languages. From 100 %, 6 Zoom-in presses reach
  250 %. At 400 % Zoom in is disabled.
- [ ] **AC2 (R3)** — Ctrl/Cmd+wheel over a control zooms in one step, and the
  control under the pointer stays under the pointer (within 1 screen px).
- [ ] **AC3 (R4, R5)** — Ctrl/Cmd+0 returns to 100 %. Fit on PowerSpatial's
  `main-form` in a small window shows the whole form at a step below 100 %.
- [ ] **AC4 (R6)** — At 200 %, a 9 px label's glyphs are rendered at 18 px
  (galley font size), not a scaled 9 px texture.
- [ ] **AC5 (R7)** — At 25 % and 400 %, selection handles measure the same on
  screen as at 100 %.
- [ ] **AC6 (R10, R11)** — At 200 %, dragging a control 40 screen px right
  with snap off moves it by 20 form px. Resizing it 40 screen px wider grows
  `w` by 20. With a 16 px snap grid, positions land on multiples of 16.
- [ ] **AC7 (R11)** — At 400 %, a click with 2 screen px of jitter on a
  control in a Grid container commits nothing (the click-is-not-a-drop rule,
  1.80.196).
- [ ] **AC8 (R12)** — Zooming in and out several times leaves the form clean
  (no dirty marker, nothing to undo), and its `.cfrm` is byte-identical.
- [ ] **AC9 (R13)** — Two forms open, one at 200 %, the other at 100 %: each
  keeps its own zoom, and a reopened form starts at 100 %.
- [ ] **AC10 (R14, R15)** — At 50 %, dragging the form's grip 100 screen px
  wider grows the form's design width by 200. The View at bar shows the design
  size, and the designer OS window does not change size.
- [ ] **AC11 (R9)** — The forms-engine suite
  (`cargo test -p cobolt-forms --features render`) passes unchanged.
- [ ] **AC12 (non-goal)** — Run Form of a form last edited at 300 % opens at
  its design size.

## 6. Constraints & steering check
- **i18n:** Zoom in / Zoom out / Fit / reset-to-100 % tooltips are new `Tr`
  fields, in all six languages.
- **Generated code / regenerate contract:** none. Zoom is view state.
- **Runtime / hosts:** none. Nothing outside the designer canvas changes. The
  shared renderer may gain a scale parameter, which defaults to 1 for every
  other surface.
- **System KB:** no control, property, method or event changes.
- **Docs:** `docs/developers-guide-en.md` gets a short "Zooming the canvas"
  passage (buttons, shortcuts, "zoom never changes your form") with a
  screenshot placeholder. Its five translations are deleted per GOLDEN RULE #8.
- **Rounded corners:** zoomed corners must still be drawn, not repaired, by
  the layered corner system. Read the `rounded-corners` skill during `/plan`.
- **Classification:** **feature** (a new IDE capability). It goes on the
  `features` line and bumps `z` only.
- **Interaction with spec 088:** grid-line dragging, cell selection and the
  floating grid toolbar follow R7/R10 when 088 lands. The two specs can be
  built in either order.

## 7. Open questions
- **Q1:** Should a form remember its last zoom when reopened (stored in IDE
  preferences, never in the `.cfrm`)? *Proposed: no, always 100 % (R13).*
- **Q2:** Is 25 %–400 % the right range? *Proposed: yes.*
