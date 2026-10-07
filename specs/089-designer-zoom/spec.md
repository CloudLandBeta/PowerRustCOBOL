# Spec — The Form Designer magnifier

- **Status:** implemented
- **Folder:** specs/089-designer-zoom/
- **Author:** operator request, written by Claude   **Date:** 2026-10-05, redesigned 2026-10-06

> **Redesigned 2026-10-06 (operator):** *"Instead to zoom in the entire form,
> place a 150x150 navigation zoom that is enabled by a rad toolbar button
> (toggle a magnifier selected/deselected) and show a little window with a 4x
> magnification focused on the mouse pointer over the form only. The
> navigation zoom would be placed on the top of the toolbox right above common
> section a new section (Zoom) if the magnifier is selected."* The first
> draft — a zoomable canvas from 25 % to 400 % — is withdrawn; the canvas
> keeps painting the form at its own size. Then: *"instead of 150x150, make
> it occupy the width of the toolbox, always a square."*

## 1. Overview
The Form Designer paints a form at exactly its own size. Small controls,
controls packed close together (PowerSpatial's cards hold 9 px labels a few
pixels apart) and the handles around them are hard to read. This feature adds
a **magnifier**: a square view at the top of the toolbox, as wide as the toolbox, that shows, four
times larger, the part of the form under the mouse pointer. It is a way of
looking at the form; the canvas, the form and every gesture on the canvas are
unchanged.

## 2. Goals / Non-goals
- **Goals**
  - One toolbar toggle turns the magnifier on and off.
  - While on, a **Zoom** section at the top of the toolbox shows the form under
    the pointer at 4×, sharp, following the pointer as it moves.
- **Non-goals**
  - No zoom of the canvas itself, no change to the form, its size, Preview,
    Run Form or built applications.
  - The magnifier is not interactive: it is looked at, not clicked or dragged
    in.

## 3. User stories
- As a form designer, I want to see a crowded card magnified while I point at
  it, so that I can tell which label and which handle my pointer is on.
- As a form designer, I want to turn the magnifier off when I do not need it,
  so that the toolbox keeps its space.

## 4. Requirements (EARS)
- **R1 (ubiquitous):** The Form Designer's icon toolbar shall carry a
  **Magnifier** toggle button (a magnifying-glass icon) that reads as selected
  while the magnifier is on, with a translated tooltip.
- **R2 (state):** While the magnifier is on and the toolbox is expanded, the
  toolbox shall show a **Zoom** section as its first section, directly above
  **Common**, holding a **square view as wide as the toolbox** (its side follows the toolbox when the developer resizes it). While it is off, the section is absent
  and the toolbox is exactly as today.
- **R3 (state):** While the pointer is over the form on the canvas, the view
  shall show the canvas at **4×**, centred on the pointer, with a small
  crosshair at the centre. It follows the pointer as it moves.
- **R4 (state):** When the pointer leaves the form, the view shall stop
  updating and keep showing what it last showed. Until the pointer has been
  over the form, the view shows a short translated hint ("Point at the form").
  *(Operator, 2026-10-06: "if the mouse leaves the form, the magnifier stops
  updating".)*
- **R5 (ubiquitous):** Only the form is magnified: whatever lies outside the
  form's rectangle (the canvas margin, the IDE around it) is not shown.
- **R6 (ubiquitous):** Magnified text shall be laid out at 4× its size, so it
  stays sharp rather than being an enlarged bitmap; strokes, borders, corner
  rounding and images are drawn at 4× too.
- **R7 (constraint):** The magnifier shall never change the form: turning it on
  or off, or pointing with it, does not mark the form dirty, add an undo step
  or alter the saved `.cfrm`. The canvas paints exactly as before.
- **R8 (ubiquitous):** The magnifier is per designer window and every form opens
  with it off.
- **R9 (constraint):** The Zoom section takes its height out of the toolbox's
  own space; the sidebar and the designer window never change size because of
  it (GOLDEN RULE: a window never resizes itself).

## 5. Acceptance criteria
- [x] **AC1 (R1, R8)** — The toolbar shows the Magnifier toggle with a tooltip in
  all six languages; it reads as selected when on; a new form opens with it off.
- [x] **AC2 (R2, R9)** — With it on, the toolbox's first section is Zoom, above
  Common; with it off there is no Zoom section. The sidebar's height is the
  same either way.
- [x] **AC3 (R3, R6)** — Pointing at a 9 px label shows that label in the view
  laid out at 36 px (galley font size), centred on the pointer.
- [x] **AC4 (R4, R5)** — Before the pointer has been over the form the view
  shows the hint; after it leaves the form the view keeps its last picture;
  near the form's edge, nothing outside the form appears.
- [x] **AC5 (R7)** — Toggling and pointing leave the form clean: no dirty mark,
  nothing to undo, byte-identical `.cfrm`.
- [ ] **AC6** — `cargo test -p cobolt-forms --features render` passes unchanged.

## 6. Constraints & steering check
- **i18n:** the tooltip, the section name and the hint are new `Tr` fields in
  all six languages.
- **Generated code / runtime / System KB:** none — designer view only.
- **Docs:** `docs/developers-guide-en.md` gets a short "The magnifier" passage
  with a screenshot placeholder; its five translations are deleted (GOLDEN
  RULE #8).
- **Classification:** **feature** (a new IDE capability) — `features-spec-089`,
  `z` bump.
