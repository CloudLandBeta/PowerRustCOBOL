# Spec — `PerspectiveStrength`: how strongly a tilted control converges

- **Status:** implemented in 1.91.144 (the review gates of /specify, /plan and /tasks are taken together: one property, one request, no open design choice that changes what the operator asked for)
- **Folder:** specs/097-perspective-strength/
- **Author:** operator request, written by Claude   **Date:** 2026-10-14
- **Branch:** `features-perspective-3d` (continues spec 096; `features` is held by another worktree)

## 1. Overview
Spec 096 gave the display controls `RotateX`, `RotateY` and `PerspectiveDistance`. The operator laid a
tilted chart over the glass panel of a background picture and **could not make it fit**: the chart's near
edge came out far larger than the panel's, and no setting of the three properties changes that without
also changing the chart's size. The reason is spec 096 R5, which made `PerspectiveDistance` act on **both**
the apparent size and the strength of the convergence: a control that is farther is smaller *and*
flatter, so the strength cannot be lowered while the size is kept.

This spec adds a fourth property, **`PerspectiveStrength`**, that scales **only the convergence** — how
much the nearer edge outgrows the farther — and leaves the size to `PerspectiveDistance`.

| Property | Meaning | Values | Default |
|---|---|---|---|
| `PerspectiveStrength` | how strongly a tilt converges; 100 is the perspective the distance gives, 0 is none (the opposite edges stay equal) | whole number, 0 … 100 | 100 |

Facts the requirements rest on, each read in the source on 2026-10-14:

1. The projection is `screen = c + (x₂, y₁) · F / (D + z₂)` with `F = K·M`, `D = F · 2^((50 − d)/50)`, and
   `z₂` the depth the tilt gives a point (`perspective.rs`, `Camera::project`). The *size* of a flat
   control is `F / D` (z₂ = 0) and depends on the distance alone; the *convergence* is the term `z₂`
   in the divisor.
2. A property seeded in `Control::new` is stored, settable from COBOL, listed by IntelliSense and
   round-tripped through the `.cfrm`; the pane, the help table, the System KB and the Guide are separate
   tables that all name the three properties of spec 096 (`model.rs`, `properties.rs`,
   `prop_help_data.rs`, `ui_text_data.rs`, `cobolt-compiler/src/lib.rs`, `docs/developers-guide-en.md`).
3. Every COBOL write to a property passes through `property_runtime_text`, which already clamps the
   three properties of spec 096 (R12 there), so a fourth is clamped in the same place for all three hosts.

## 2. Goals / Non-goals
- **Goals:** one property, on the same 12 types, that lowers (down to none) the convergence of a tilt without
  changing the size; a control that does not use it is drawn **exactly** as before.
- **Non-goals:** strengthening the convergence beyond what the distance gives (the distance does that, and a
  stronger term could put a point behind the camera); an offset of the vanishing point; any change to
  `RotateX`, `RotateY` or `PerspectiveDistance`.

## 3. User stories
- As a **dashboard designer**, I want to keep a tilted chart the size it is and soften its perspective, so that
  it fits the glass panel of my background picture.
- As a **COBOL programmer**, I want `MOVE 40 TO CHART-1::PerspectiveStrength`, so that a handler can ease a
  tilt in or out like any other property.

## 4. Requirements (EARS)
- **R1 (ubiquitous):** The system shall give `PerspectiveStrength` to exactly the 12 control types of spec 096
  R1 — whole number 0 to 100, default 100.
- **R2 (ubiquitous):** `PerspectiveStrength` shall scale **only the convergence**: it shall not change the
  position of the control's centre, the size of a control that is not tilted, or the angle of the tilt.
- **R3 (ubiquitous):** For a fixed non-zero tilt and a fixed distance, the ratio of the nearer edge to the farther
  shall **decrease strictly** as the strength decreases; at 0 the two edges shall be equal in length (the opposite
  edges stay parallel and equal: no convergence).
- **R4 (state):** While `PerspectiveStrength` is 100 or absent, a control shall be drawn **exactly** as it was
  before this property existed, and a form saved before shall load with it absent (R13 of spec 096).
- **R5 (event):** When COBOL writes a value outside 0 … 100 to `PerspectiveStrength`, it shall be clamped into the
  range and read back clamped, in all three hosts (R12 of spec 096).
- **R6 (ubiquitous):** For every value of the four properties the projected outline shall be a convex quadrilateral
  and nothing shall lie behind the camera (R5 of spec 096's footing).
- **R7 (ubiquitous):** The Properties pane shall show the property for the 12 types with a help text in the six IDE
  languages; IntelliSense shall list it; the System KB shall document it (store regenerated); the Developer's Guide
  shall describe it with a picture of the three strengths; a control of another type shall not offer it.
- **R8 (constraint):** The property shall not be theme-owned and shall not change a control's layout rectangle,
  tab order or event bindings.

## 5. Acceptance criteria
- [x] **AC1 (R1, R4, R8)** — the 12 types seed `PerspectiveStrength` 100; no other type seeds or lists it; a form
  saved without it loads without it and saves byte for byte as before; a theme switch leaves it as it was.
- [x] **AC2 (R3)** — for `RotateY` −30 at distance 50 the far/near edge ratio over strengths 0, 25, 50, 75, 100 is
  printed and **falls strictly as the strength falls**; at 0 the two edges are equal; at 100 it is the 0.778 spec 096 measured.
- [x] **AC3 (R2)** — the centre does not move for any strength; an untilted control is the same size at every
  strength; the width of the outline at distance 100 shrinks as the strength falls while distance 50 → 100 still
  doubles an untilted control.
- [x] **AC4 (R4)** — at strength 100 the outline of every tilt and distance of a sweep equals, to 1e-4 px, what the
  three-property projection gave before; the corpus goldens pass unregenerated.
- [x] **AC5 (R5)** — `MOVE 150 TO …::PerspectiveStrength` reads back 100 and `MOVE -5` 0, under Run Form, as a child form
  and in a built binary.
- [x] **AC6 (R6)** — over every whole degree of both angles × every distance × strengths 0, 25, 50, 75, 100: the
  divisor stays positive and every outline is convex.
- [x] **AC7 (R7)** — the pane shows the property for each of the 12 types in six languages with no English left;
  IntelliSense lists it; `prebuilt_chunked_kb_matches_the_published_documentation` is green; the picture of the
  three strengths in the Guide has been **looked at**.

## 6. Constraints & steering check
- **i18n ×6:** yes — the row label (`ui_text_data.rs`) and the help text (`prop_help_data.rs`) in six languages.
- **Generated code / regenerate contract:** no impact (codegen reads no property).
- **Docs:** the English Developer's Guide only; no translation exists to delete.
- **System KB:** yes — the property arm and `build_chunked_kb`.
- **Fix vs feature:** **feature** (a new property). Version `z` bump, CHANGELOG entry.

## 7. Open questions (each taken at the answer assumed here)
- **Q1 — Range.** Strength above 100 (stronger than the distance gives) was not asked for and could put a point
  behind the camera at the nearest distance. *Assumed:* 0 … 100 only; the distance is the way to strengthen.
- **Q2 — Default.** 100, so that every form that exists is drawn as it was (R4). *Assumed.*
- **Q3 — Name.** `PerspectiveStrength`, beside `PerspectiveDistance`. *Assumed.*
