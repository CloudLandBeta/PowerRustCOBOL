---
name: egui-paint-regressions
description: Post-mortem playbook for egui upgrade rendering/UI regressions in PowerRustCOBOL — corner-arc bleed (u8 radius quantization vs StrokeKind), self-inflating Resize windows, self-closing popups — with the proven diagnosis method (shape-dump diffing) and the exact fixes. Read when panels/containers show dark or light arcs at rounded corners, when a window/modal grows on its own, when a popup opens and instantly closes, or before ANY future egui version bump.
---

# egui paint & UI regressions — what broke in 0.29→0.35 and how it was fixed

All three regression families below shipped during the egui 0.35 upgrade
(spec 027, July 2026), each looked mystifying on screen, and each has a
one-line root cause once you know it. **Do not re-derive these from scratch.**

## 1. Corner-arc bleed: dark or light arcs hugging rounded corners

**Symptom:** thin arcs following the corner curvature of rounded panels /
GroupBoxes — dark arcs (Neumorphic borders) or light arcs (Classic/Enhanced
glass over a background image). Straight edges always clean. Worse on
Preview/Run Form (transparent viewports) than the designer (opaque theme
hides half-pixel errors).

**Root cause:** egui ≥ 0.31 stores corner radii as **integer `u8`**
(`CornerRadius`). The pre-0.31 concentric-stroke idiom
`rect.shrink(half) + radius (face_r − half) + StrokeKind::Middle`
needs **fractional radii** (e.g. 23.5 for face 24, stroke 1). u8 cannot
express them, and **no rounding direction works**:
- round **up** → the stroke's corner arc bulges *outside* the face; its dark
  outer half shows against the surround → **dark arcs**;
- round **down** (floor) → the corner-notch mask's repainted backdrop pokes
  out past the (now tighter) restored rim → **light arcs**.

**The fix (exact, no fractional radius needed):** paint concentric strokes at
the **full face rect, the exact integer face radius, `StrokeKind::Inside`**.
Inside placement keeps the whole stroke width within the rect — outer edge ==
face edge, geometrically exact in u8.

```rust
// WRONG (pre-0.31 idiom, cannot survive u8 radii):
painter.rect_stroke(rect.shrink(half), face_r - half, stroke, StrokeKind::Middle);
// RIGHT:
painter.rect_stroke(rect, face_radius, stroke, StrokeKind::Inside);
```

**Fills are different:** soft shadow/glow layers (radius = r + fractional
expand, see `round_map` in `cobolt-forms/src/paint.rs`) must use
**round-to-nearest** — flooring makes every layer systematically squarer,
which bands dark exactly on the corner diagonals.

**Regression guards:** `concentric_border_arcs_stay_inside_the_face` and the
scene dumps in `cobolt-forms/src/render.rs` (`shape_dump` test module).

## 2. Diagnosis method that actually worked: shape-dump diffing

Screenshots + hypotheses wasted several round-trips. What pinpointed both
corner bugs in one shot: **render the identical scene on the old and new egui
and diff the emitted `FullOutput.shapes`** (pre-tessellation, so radii/rects/
colors/clips are directly comparable).

- Harness: `cobolt-forms/src/render.rs` → `shape_dump` tests, env-gated
  (`COBOLT_SHAPE_DUMP=<file>` scene A neumorphic, `COBOLT_SHAPE_DUMP_B=<file>`
  scene B classic glass + backdrop image + corner child). Add a scene per new
  artifact; run on both versions (old via a throwaway `git worktree`), diff.
- The whole 0.29 vs 0.35 diff for the corner bug was TWO numbers (23.5 → 24).
- Also useful: `COBOLT_FRAME_DIAGNOSTICS=1` labels each paint frame on screen.

## 3. Self-inflating windows/modals (Resize ratchet)

**Symptom:** a resizable window/modal grows every frame until it fills the
screen.

**Root cause:** egui ≥ 0.35 `Resize` does
`desired_size = desired_size.max(measured_content_min)` **every frame**. Any
body that overflows the box — e.g. a layout with an *estimated* footer height
that font-metric changes (skrifa, 0.34) made 2px too small — ratchets forever.

**Fix:** never estimate interior heights. Partition the fixed box with
embedded panels (`egui::Panel::bottom` for the button row, `CentralPanel` for
the scrollable content) so measured content == box **exactly** regardless of
font metrics. See `error_modal_scaffold`/`error_modal_body_ui` in
`cobolt-ide/src/app.rs` and the 120-frame test
`error_modal_holds_seeded_size_across_frames`.

The older sibling rule still holds (memory `egui-resize-autogrow`): never size
a child from available/remaining space inside an auto-sizing container.

## 4. Popups that open and instantly close

**Symptom:** a hand-rolled popup (raw `Area` + manual open state) opens on
click and closes by itself one frame later (e.g. the properties color picker).

**Root cause:** since egui 0.32 the popup manager **force-closes any popup id
that is not re-registered through the `Popup::show` API each frame**.
Registering via `Popup::toggle_id`/`is_id_open` but drawing a raw `Area`
counts as "not shown" → killed after one frame.

**Fix:** either use the real `egui::Popup` API end-to-end, or keep the popup's
open flag **yourself** (a bool in `ui.memory` temp data) and don't touch the
popup manager at all. PowerRustCOBOL's color picker does the latter — see
`color_edit_button_closing` in `cobolt-ide/src/panels/properties.rs`.

## Checklist for the next egui bump

1. Step one minor at a time; fix deprecations at each step (0.34 deleted all).
2. grep for `shrink(half)`/`- half` near `rect_stroke` — any survivor is a
   corner-bleed candidate; convert to `StrokeKind::Inside`.
3. Run the `shape_dump` scenes against the previous version before trusting
   your eyes.
4. Re-run the modal 120-frame test and the concentric-arc guard.
5. Check every hand-rolled `Area` popup still opens.
