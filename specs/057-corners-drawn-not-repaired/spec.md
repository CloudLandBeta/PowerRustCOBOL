# Spec — Corners are drawn rounded, not repaired

- **Status:** draft → awaiting review
- **Folder:** specs/057-corners-drawn-not-repaired/
- **Author:** Claude (Opus 5) with the operator   **Date:** 2026-09-05

> **The operator's framing, which is the whole point of this spec:** *"we are
> fighting the effects, not the cause. Locate the code that paints the frames
> and make sure it draws the rounded border from the beginning. If more than
> one function paints a frame, make sure all of them implements the corner
> radius defined for the control."*

## 1. Overview

A rounded container's corners are painted **twice**: once correctly, then once
wrongly.

Children of a rounded Panel/GroupBox already draw themselves to the parent's
arc. `render.rs` seeds a transient `_ContainerClip` on every visual child (§7
E3), and `draw_control` turns it into `frame_round` — the child's own radius
**lifted to the container's radius** on any corner that lands on the parent's
border (E4/E5). The chart, the images, the widget faces all receive that value.
By the time a corner is finished, it is already round.

The engine then runs the **corner-notch mask** over it. The mask fires on one
condition only — "this container has a descendant whose rect reaches this
corner" (E6) — and never asks whether that descendant already clipped itself.
It repaints the **form backdrop** into the notch (E8). That is correct only when
the form backdrop is what is actually behind the container, and in the operator's
own form it is not:

```
PictureBox-1  @(0,208)    1720x784      ← what is really behind the panel
PictureBox-2  @(-288,72)  1968x848
Panel-1       @(440,792)   592x496      Transparency=100, CornerRadius=26
```

The mask paints the form's `assets/rio0.png` (Stretch, 1672×888) into the four
corners while the surrounding pixels show the PictureBox's image at a different
rect. Same picture, different scale and offset — a visibly mismatched wedge at
every corner, on the designer, the preview and the run form alike. It reads as
a transparency bug, which is why it has been chased as one for a long time.

Everything downstream of the mask — `restore_container_outline`, and the
offscreen GL clip — exists to repair damage the mask causes. This spec removes
the cause: **a corner that was drawn rounded is never repainted.**

It also records a fact found while diagnosing this, which invalidates the
standing "cure" in the notes: **the GL rounded clip cannot run on this build.**
It is written against `egui_glow::CallbackFn` (E10) while eframe's default
feature resolves to **wgpu** (E11). Its callbacks are never invoked, so the
`rounded_clip` switch silently disables the working fallback and substitutes
nothing.

## 2. Goals / Non-goals

**Goals**

- G1 — A container corner whose overlapping descendants all clip themselves is
  **not masked**, on every surface.
- G2 — The visible result over any backdrop — a form colour, a form image, a
  PictureBox, another control, a translucent surface — is the container's own
  arc with whatever is genuinely behind it showing through. No repainted
  backdrop, no wedge.
- G3 — Every function that paints a control frame uses the corner radius the
  control resolves to, including the ones that do not today (§4.4).
- G4 — The mask survives only where it is still needed: content that provably
  cannot clip itself. Which types those are is decided by **measurement**, not
  assertion (R7).
- G5 — The dead GL path stops misleading its next reader.

**Non-goals**

- Porting the offscreen clip to wgpu. If G1–G2 hold, nothing needs it; if a
  residual case survives R7's measurement, that is a separate spec with its own
  evidence.
- Changing `_ContainerClip`, `container_image_rounding` or the lifting rule —
  they are the part that works.
- Nested-container masking policy beyond what falls out of G1.
- Any change to how Transparency composites.

## 3. User stories

- As a **form developer**, I put a chart in a rounded panel over a picture and
  the corners look like the panel's corners — not like four patches of a
  differently-scaled image.
- As a **form developer**, I set `Transparency` on a container and its corners
  behave exactly as its edges do.
- As a **maintainer**, when a corner is wrong I can tell which painter drew it,
  because only one did.

## 4. Requirements (EARS)

### 4.1 The rule

- **R1 (ubiquitous):** The system shall treat a container corner as **already
  correct** when every descendant whose rect overlaps that corner carries a
  container clip (`_ContainerClip`) for that container.
- **R2 (state):** While a corner is already correct, the system shall not paint
  the corner-notch mask over it, and shall not restore an outline on it.
- **R3 (ubiquitous):** The system shall decide R1 per corner, not per container:
  a container may have one corner reached by self-clipping content and another
  reached by content that cannot clip.
- **R4 (ubiquitous):** The existing guardian shall keep its current duty — a
  corner **no** descendant reaches is still never masked — so R1 narrows the
  masked set and never widens it.

### 4.2 What still needs the mask

- **R5 (ubiquitous):** The system shall keep the notch mask for a corner reached
  by content that does not carry a container clip.
- **R6 (ubiquitous):** Where the mask does run, it shall keep its current
  behaviour unchanged — backdrop colour, gradient, image (including the tile
  under that corner), and the control's own shadow re-composited at the alpha it
  was drawn with.
- **R7 (constraint):** Which control types can clip themselves shall be
  established by a **rendered measurement**, one per type, placed at a container
  corner and checked for paint outside the arc — not by reading
  `clips_to_container_border` and trusting it. The measurement's results are
  part of this feature's deliverable and shall be recorded in `plan.md`.

### 4.3 Surfaces

- **R8 (ubiquitous):** R1–R6 shall hold identically on the designer canvas, the
  preview and the run form. A corner shall not differ between them.
- **R9 (ubiquitous):** The rule shall be implemented in ONE function that all
  surfaces call, beside the existing guardian, so the two cannot drift.

### 4.4 Every frame painter uses the control's radius

- **R10 (ubiquitous):** Every function that paints a control's frame shall draw
  it with the radius that control resolves to. The audit in §7 found two that do
  not, and both shall be corrected:
  - the **selection stroke** (E12), drawn square on a rounded control;
  - the **container shadow** (E13), which uses the control's own radius rather
    than the lifted one, so a child's shadow at a parent corner is shaped
    against the wrong arc.
- **R11 (constraint):** Any painter added later that draws a control frame shall
  take its radius from the same resolution path; a new frame drawn square is a
  defect, not a style choice.

### 4.5 The dead GL path

- **R12 (ubiquitous):** The system shall not present a control that does
  nothing. The `rounded_clip` switch shall either be removed, or report that the
  offscreen clip is unavailable on this rendering backend and change no
  behaviour when set.
- **R13 (ubiquitous):** The module's documentation shall state that it targets
  the glow backend and that the shipped build resolves to wgpu, so its next
  reader is not misled as this session's was.

### 4.6 Process

- **R14 (constraint):** No artifact of this feature shall assert how a painter
  behaves without a file:line or a rendered measurement. The corner system has
  cost this project repeatedly, and every past recurrence was one layer's
  assumption about another.
- **R15 (constraint):** `/plan` shall re-read every citation in §7 and shall
  enumerate, by reading, every caller of the mask, the guardian and the restore.

## 5. Acceptance criteria

- [ ] **AC1 (R1, R2, G2)** — `inner-form2` renders headlessly on all three
  surfaces with **no notch mask and no restore outline** on Panel-1's four
  corners; the pixels there come from the PictureBox behind it, not from the
  form's own background image.
- [ ] **AC2 (R4)** — A corner no descendant reaches is still unmasked
  (`corner_notch_guardian_*` stays green, unmodified).
- [ ] **AC3 (R3)** — A container with one corner reached by a self-clipping
  child and another reached by non-clipping content masks exactly one corner.
- [ ] **AC4 (R5, R6)** — For a corner that still needs the mask, the painted
  result is byte-identical to today's (a golden captured before the change).
- [ ] **AC5 (R7)** — One rendered measurement per control type, at a container
  corner, recording whether it paints outside the arc. The list of types that
  need the mask is derived from those results and cited in `plan.md`.
- [ ] **AC6 (R8, R9)** — The three surfaces produce the same corner decision for
  the same form, asserted by one test that drives all three.
- [ ] **AC7 (R10)** — A selected rounded control's selection stroke follows its
  arc; a child's shadow at a parent corner is shaped to the parent's arc.
- [ ] **AC8 (R12, R13)** — Setting `rounded_clip` on the wgpu build changes no
  pixel, and the module says why.
- [ ] **AC9 (regression)** — The corner guards that exist today all stay green,
  including `a_faded_container_restores_a_faded_rim`,
  `concentric_border_arcs_stay_inside_the_face` and the DataGrid arc guards.
- [ ] **AC10 (G2)** — A rounded translucent container over each of: a form
  colour, a form image, a PictureBox, another control. No wedge in any.

## 6. Constraints & steering check

- **i18n:** none — no new user-visible strings, unless R12 chooses to label the
  switch, in which case ×6.
- **Generated COBOL:** unaffected.
- **System KB:** no control property changes; the corner behaviour described in
  the KB should be re-read at `/docsync` for statements that assume masking.
- **Docs:** the `rounded-corners` skill and its `CORNER-BLEED-PLAYBOOK.md` must
  be updated — they currently name the GL path as the cure for translucent
  surfaces, which is unreachable on this backend. That correction is part of
  this feature, not a follow-up.
- **Fix vs feature:** **fix**. It removes an artifact and deletes work; nothing
  new is offered. Fix number bump; `fixes` branch.
- **Verify-first:** AC1 and AC5 are measurements, not inspections.

## 7. Code read for this spec (evidence; `/plan` re-reads every one)

| # | Fact | Where |
|---|---|---|
| E1 | The reproduction ships in the repo: a translucent rounded Panel over two PictureBoxes, holding three Gauges and a LineChart | `examples/PowerDemo3/forms/General/inner-form2.cfrm` |
| E2 | That panel: `CornerRadius=26`, `Transparency=100`, gradient `#F1F1F125`→`#9F9F9FFF`, `BorderStyle=Fixed3D`, shadow on | same file, `Panel-1` |
| E3 | Every visual child of a rounded Panel/GroupBox is given a container clip; only Timer, AgentObject, SqlDatabase and RestClient are excluded | `crates/cobolt-forms/src/render.rs:644-652`, set at `2102` and `2603` |
| E4 | `control_border_rounding` lifts the parent's radius onto a child's own frame corners | `crates/cobolt-forms/src/paint.rs:11436-11444` |
| E5 | The main control frame uses it: `frame_round = control_border_rounding(ctrl, frame_rect, corner)` — and hands the same value to the chart | `paint.rs:4095`, chart call at `5718` |
| E6 | The mask fires on descendants-reach-the-corner alone, with no self-clip test | `render.rs:760-777` (`notch_mask_rounding`), `779` (`corner_notch_rounding`) |
| E7 | The mask runs over the whole effective control list each frame | `render.rs:826-900` (`mask_container_notches`) |
| E8 | The mask repaints the FORM backdrop — colour, gradient, image, tile — into the notch | `paint.rs:11188-11240` (`draw_container_notch_mask`) |
| E9 | The restore exists only to redraw what the mask overpainted | `paint.rs:11284-11340`; callers `render.rs:904, 9513, 9599` |
| E10 | The offscreen clip is an OpenGL implementation: `egui_glow::CallbackFn`, `gp.gl()` | `crates/cobolt-ide/src/panels/rounded_clip.rs:199, 260` |
| E11 | The build resolves eframe's default feature to **wgpu**, so those callbacks never run | `cargo tree -p cobolt-ide -e features`: `eframe feature "default" → eframe feature "wgpu"` |
| E12 | A selected control's stroke is drawn square (`0.0`) on a rounded control | `paint.rs:4260` |
| E13 | The container shadow uses the control's OWN radius, not the lifted one | `paint.rs:2252-2256`, and `drop_shadow_corner_radius` at `9605` |
| E14 | The GL path was introduced as the cure for exactly this bleed, opt-in and designer-only | `CHANGELOG.md`, 1.27.140 — 2026-07-08 |
| E15 | The mask was already narrowed once, to corners a child actually reaches | `CHANGELOG.md`, 1.27.141 (“corner guardian”) and 1.27.138 |

**Not read, and therefore not asserted** (the plan must read them): how the
designer's `render_faces` path reaches the mask; whether `Surface::Pane` changes
the backdrop the mask paints; the DataGrid's own arc banding, which is a
different mechanism and must not be disturbed.

## 8. Open questions

- **Q1 — Delete the mask for self-clipping corners, or paint it only where the
  child is provably unable to clip?** The spec assumes the second (R5), which is
  strictly safer. R7's measurement may show the first is achievable, which would
  let the mask, the restore and the GL module all be deleted.
- **Q2 — Remove the `rounded_clip` switch, or keep it inert with an
  explanation?** R12 allows either; removal is cleaner, keeping it documents the
  history.
- **Q3 — Should `Transparency` fade a container's user border?** Out of scope
  here, but noticed while diagnosing: `draw_control` paints the border at full
  strength on a fully transparent container, which is why 1.65.1 deliberately
  does not fade it in the restore either.
