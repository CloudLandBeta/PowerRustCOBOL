# Spec — Universal border radius + rounded content clipping

- **Status:** draft → approved
- **Folder:** specs/016-universal-border-radius/
- **Author:** Anthropic Claude Codex Agent (with Eslopes)   **Date:** 2026-06-21

## 1. Overview

Today only some controls expose a corner-radius property — containers carry
`BorderRadius` (spec 012) and `Button` carries `CornerRadius`; every other
control draws with a fixed, hard-coded corner. This feature makes **every visual
control that draws a border** expose a single, consistent **border-radius**
property and **clip its content to the rounded border**. A radius of **0** means
square corners and **no clipping** (today's look); a positive radius rounds the
frame *and* trims any content that would spill past the rounded edge. The rule is
uniform across the Form Designer canvas, the live preview, the running
(interpreted) form, and compiled/web builds, because all four share
`cobolt_forms::paint::draw_control`.

**Important technical note (drives the plan):** egui 0.29's runtime clip is only
an **axis-aligned rectangle** (`Painter::with_clip_rect` / `ClippedPrimitive.clip_rect`
is a `Rect`) — there is **no arbitrary-shape or rounded-rectangle scissor**.
However, content can still be rounded by baking the radius into the *geometry*:
- a control's **fill + border** round trivially (`RectShape.rounding`, already used
  via the `corner` value in `draw_control`);
- a **raster image** rounds **natively over any background** by drawing it as an
  `epaint::RectShape` with `rounding` + `fill_texture_id` + `uv` (confirmed on the
  pinned egui 0.29.1; the `egui::Image::rounding()` widget is the `Ui`-level
  equivalent). PictureBox today uses `painter.image()` (a `RectShape` *without*
  rounding) — adding `rounding` is the whole fix for images;
- **mesh content** (charts) rounds by clipping its mesh geometry.
Only **non-image, non-mesh content** — egui native `TextEdit`/`ScrollArea` layers
and arbitrary child widgets — cannot be shape-masked; see §7 Q3.

## 2. Goals / Non-goals

### Goals
- A single **border-radius** property on **every visual control that has a
  border/background frame**, editable in the properties pane.
- The control's **fill and border render rounded** to that radius.
- The control's **content is clipped to the rounded shape**; radius 0 ⇒ square +
  unclipped (unchanged from today).
- Identical behaviour in **designer, preview, run, and compiled** outputs (one
  shared renderer).
- A consistent property **name** and behaviour across all controls (folding in
  the existing container `BorderRadius` and Button `CornerRadius`).

### Non-goals
- Per-corner radii (one uniform radius per control for v1).
- Rounding non-bordered/again-non-visual controls (Label text, Line, Timer,
  AgentObject, SqlDatabase, RestClient — see §4 for the exact set).
- Changing the existing container clipping model beyond making it honour the
  rounded shape (spec 012 stays the containment source of truth).
- Drop-shadow shape changes (the existing shadow keeps its current look).

## 3. User stories
- As a form designer, I set a TextBox's border radius to 10 and it shows rounded
  corners both on the canvas and when I run the form.
- As a form designer, I round a PictureBox and the image is trimmed to the
  rounded frame instead of poking out at the corners.
- As a form designer, I leave a control's radius at 0 and it looks exactly as it
  does today (square, no clipping).
- As a form designer, the rounded look is identical in the designer, the preview,
  and the running form.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** every **bordered visual control** shall expose a
  **border-radius** property (integer pixels, default **0**). The set of bordered
  visual controls is: `Button`, `TextBox`, `ComboBox`, `ListBox`, `ListView`,
  `PictureBox`, `DataGrid`, `NumericUpDown`, `DateTimePicker`, `ProgressBar`,
  `GroupBox`, `Panel`, `TabControl`, `Slider` (track), `Shape` (rectangle), and
  the chart controls (`BarChart`, `LineChart`, `PieChart`, `AreaChart`,
  `ScatterChart`, `DonutChart`). *(Label, Line, MenuBar/ToolBar/StatusBar, and
  non-visual controls are excluded — see Q4.)*
- **R2 (ubiquitous):** the control's **background fill and border** shall be drawn
  rounded to the border-radius.
- **R3 (state):** while border-radius **> 0**, the control's **content** (text,
  image, child controls, chart marks, grid rows, …) shall be **clipped to the
  rounded border**; while border-radius **= 0**, no rounding and **no clipping**
  shall be applied (identical to today).
- **R4 (ubiquitous):** the radius shall be **clamped** so it never exceeds half
  the smaller of the control's width/height (no degenerate shapes).
- **R5 (constraint):** the property shall be **one consistent name and meaning**
  across all controls; the existing container `BorderRadius` and Button
  `CornerRadius` shall be unified under it (Q1), preserving backward-compatible
  load of old `.cfrm` files.
- **R6 (ubiquitous):** designer, preview, running form, and compiled/web builds
  shall all honour the radius and clipping identically (shared `draw_control`).
- **R7 (constraint):** existing forms shall be **visually unchanged** where the
  radius resolves to its current effective value (defaults chosen so nothing
  shifts — Q2).
- **R8 (constraint):** new UI strings shall be `Tr` in all six languages; the
  English dev guide shall document the property; translations untouched.

## 5. Acceptance criteria
- [ ] **AC1 (R1,R5)** — Every control in the R1 set shows a single border-radius
  row in the properties pane; old `.cfrm` files with `BorderRadius`/`CornerRadius`
  still load and map to it.
- [ ] **AC2 (R2)** — Setting radius > 0 rounds the fill + border of each control
  type (verified on the canvas for a representative set).
- [ ] **AC3 (R3)** — With radius > 0, content is trimmed at the rounded corners:
  a PictureBox image and a chart's marks do not poke past the rounded frame; with
  radius 0 the control looks exactly as before (square, unclipped).
- [ ] **AC4 (R4)** — A radius larger than half the control size is clamped (e.g. a
  24-px-tall field with radius 40 renders as a pill, not a glitch).
- [ ] **AC5 (R6)** — The same form renders identically (radius + clipping) in the
  designer, the preview, and the running form.
- [ ] **AC6 (R7)** — A pre-existing form opened after the change is visually
  unchanged until a radius is edited.
- [ ] **AC7 (R8)** — New strings exist in all six languages
  (`cargo test -p cobolt-ide i18n`); the English guide documents the property.

## 6. Constraints & steering check
- **i18n (6 languages):** the new "Border radius" property label is `Tr` ×6.
  *(Note: the properties pane currently uses inline literals for most rows; the
  shared label follows the steering rule.)*
- **Generated-code / regenerate contract:** rendering-only; no codegen change
  (the property serialises generically in `.cfrm`). Banner/regenerate unchanged.
- **Docs:** English `developers-guide-en.md` documents the universal property +
  the clipping rule (and any limitation from Q3); translations untouched.
- **Fix vs feature:** new cross-cutting property → normally a feature; per the
  standing pre-production directive, treated as a **fix** (patch bump + CHANGELOG)
  unless lifted before merge.
- **No "cobolt" in user text; COBOL identifiers English.**

## 7. Open questions
- **Q1 — property name (RESOLVED):** unify on **`CornerRadius`** (operator's
  choice). `CornerRadius` is the canonical key written on every bordered control;
  the container key **`BorderRadius`** (spec 012) is read as a **backward-compat
  alias** so old `.cfrm` files still round. Button already uses `CornerRadius`.
- **Q2 — default values:** default **0** for all controls so existing forms are
  unchanged. But Button currently defaults `CornerRadius = 3` and charts use a
  fixed 8-px corner — keeping their *current* look means a non-zero default for
  those. *Recommendation: preserve each control's current effective default
  (Button 3, charts 8, everything else 0), exposed through the unified property,
  so AC6 holds; the user can set 0 to square them.*
- **Q3 — rounded-clipping technique (resolved):** egui has no shape scissor, but
  most content rounds via *geometry*, so the only hard case is narrow:
  - **Fill + border** — `RectShape.rounding` (already in `draw_control`). ✅
  - **PictureBox image** — draw as a rounded textured `RectShape` (`rounding` +
    `fill_texture_id` + `uv`); native, correct over **any** background including a
    form background image. No corner-mask needed. ✅
  - **Charts / mesh content** — round the mesh geometry. ✅
  - **Residual** — egui native `TextEdit`/`ScrollArea` layers (the editable text
    of run-time inputs) and arbitrary nested widgets can't be shape-masked.
  *Recommendation: round fill/border + image (`RectShape`) + mesh content
  natively; for the rare non-image overflow on a control with a **solid** fill,
  fall back to corner-masking against that fill; accept that the editable
  text/scroll layer stays square inside a rounded face (note it). The
  background-image limitation from the earlier draft is **gone** for images.*
- **Q4 — control set:** confirm the R1 list. Should `Label` (transparent, no
  frame) and the bars (`MenuBar`/`ToolBar`/`StatusBar`) be included? *Recommendation:
  exclude Label and the bars from clipping (no real frame), include everything
  else in R1.*
- **Q5 — interactive run widgets:** TextBox/ComboBox/ListBox/etc. in the running
  form overlay native egui widgets (TextEdit, ScrollArea) that **cannot be
  rounded-clipped**; only their `draw_control` face can. Accept that the editable
  text layer stays square inside a rounded face? *Recommendation: round the face;
  accept square text-layer for v1 (note it).*
