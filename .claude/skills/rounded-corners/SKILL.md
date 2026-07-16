---
name: rounded-corners
description: THE reference for PowerRustCOBOL's rounded-corner rendering — the layered corner system (face, concentric borders, notch mask, restore outline, GL clip), every known recurring corner failure mode (dark/light corner arcs, crescents, bleed past the arc, erased rims) with root cause and exact fix, and the proven diagnosis workflow. Read BEFORE touching any code that paints, masks, clips, or strokes a rounded corner, whenever a screenshot shows artifacts at container corners, and before any egui version bump.
---

# Rounded corners in PowerRustCOBOL — the recurring problem, solved pieces

Corner artifacts have recurred across specs 016/017 and the egui 0.35 upgrade.
The system is layered and every layer must agree about the arc **to the
pixel**; each historical bug was one layer disagreeing. This file is the
accumulated knowledge — do not re-derive it, and do not "fix" one layer
without checking the others.

## The corner system (paint order, per rounded container)

All in `crates/cobolt-forms/` (`paint.rs`, `render.rs`):

1. **Radius resolution** — `paint::corner_radius(ctrl)`: canonical
   `CornerRadius` property, legacy `BorderRadius` alias, per-type default,
   clamped to half the smaller side. `Control::new` seeds `CornerRadius`, so
   setting only `BorderRadius` on a fresh control is SHADOWED (bit me in a
   test scene).
2. **Soft shadows** (glass/neumorphic) — expanded, translated `rect_filled`
   layers, radius `r + fractional expand` through `round_map`.
3. **Face fill** — the rounded rect itself, integer radius `r`.
4. **Concentric border/rim strokes** — MUST be
   `rect_stroke(FULL face rect, r, stroke, StrokeKind::Inside)`.
5. **Children** — egui clips to axis-aligned rects only, so child content
   **bleeds past the arc** into the corner notches. That bleed is expected;
   the next layers repair it.
6. **Corner-notch mask** — `mask_container_notches` →
   `draw_container_notch_mask`: repaints the form backdrop (color AND
   background image) over the notch region (inside bbox, outside arc).
   Form-level containers only; nested containers are skipped (their notches
   must reveal the parent surface, unknowable without compositing).
7. **Restore outline** — `restore_container_outline`: the mask overpaints the
   rim on the arcs, so the rim (+ user border) is redrawn clipped to the four
   corner squares.
8. **GL rounded clip** (opt-in `COBOLT_ROUNDED_CLIP=1`,
   `cobolt-ide/src/panels/rounded_clip.rs`): captures the real backdrop and
   re-blits it through an arc mask — the only correct answer over
   *translucent* backdrops and for *nested* containers.

## Known failure modes → causes → fixes

| Symptom | Root cause | Fix (all shipped) |
|---|---|---|
| **Thin DARK arcs** hugging corners (borders) | egui ≥0.31 radii are **u8**; the old idiom `rect.shrink(half) + (r−half) + StrokeKind::Middle` needs fractional radii; rounding UP pushes the stroke arc outside the face | Concentric strokes use `StrokeKind::Inside` at the full rect + integer face radius — exact, no fractional radius exists. Never reintroduce `shrink(half)` strokes |
| **Thin LIGHT arcs** at corners (mask sliver) | Same u8 problem, rounding DOWN: restored rim tighter than the mask boundary exposes masked backdrop | Same fix — Inside stroke outer edge == face edge == mask boundary |
| **Dark banding on corner diagonals** (neumorphic) | Shadow-fill radii (`r + fractional expand`) floored → every layer squarer | `round_map` uses round-to-nearest for FILLS (strokes don't go through it anymore) |
| **Transparent/discoloured CRESCENT on a clean corner** | Notch mask painted over a corner **no child reaches**, destroying the container's own arc | `corner_notch_rounding` guardian: mask only corners a descendant overlaps. BOTH call sites must route through it — never call `draw_container_notch_mask` with blanket `CornerRadius::same(r)`. Pinned by `corner_notch_guardian_*` tests |
| **Backdrop-coloured hole punched through a parent panel** | Notch mask applied to a NESTED container repaints the *form* backdrop, not the parent surface | Nested containers skip the flat mask entirely; only the GL rounded clip can fix them properly |
| **Bleed visible over translucent surfaces** | Flat mask can't reproduce a see-through backdrop | GL capture/re-blit path (`COBOLT_ROUNDED_CLIP=1`) |
| **Child content square-clipped past the arc** (e.g. images) | egui rect-only clipping | Per-child `container_image_rounding` / `_ContainerClip` lifts the child's own corner radius where it lands on the parent arc (spec 017) |

## Diagnosis workflow (what actually works — in this order)

1. **Ask which surface**: designer (opaque theme masks half-pixel errors) vs
   Preview/Run Form (transparent viewports — artifacts show). Which glass
   style — Classic/Enhanced/Neumorphic exercise different layers.
2. **`COBOLT_FRAME_DIAGNOSTICS=1`** — labels every corner painter on screen
   (`CONTAINER_NOTCH_MASK`, `CONTAINER_RESTORE_OUTLINE`, `ROUNDCLIP_*`); the
   label at the artifact names the offending layer.
3. **Shape-dump diffing** (the method that cracked the 0.35 bugs in one
   diff): env-gated tests in `render.rs::shape_dump`
   (`COBOLT_SHAPE_DUMP=<file>` neumorphic scene, `COBOLT_SHAPE_DUMP_B=<file>`
   classic + backdrop image + corner child) dump every non-text paint shape
   normalized. Render the same scene on the reference commit (throwaway
   `git worktree`) and `diff` the dumps — pre-tessellation radii/rects/clips
   compare directly. Add a scene per new artifact class.
4. **Guard tests** must stay green:
   `concentric_border_arcs_stay_inside_the_face` (stroke arcs never outside
   the face), `corner_notch_guardian_*` (clean corners untouched).

## Invariants when writing ANY new corner code

- One radius source of truth per container: `paint::corner_radius(ctrl)`.
- Strokes concentric with a face: full rect + face radius + `Inside`. Never
  derive a fractional radius; u8 cannot hold it.
- Fill radii derived with fractional math go through `round_map`
  (round-to-nearest).
- The notch mask must exactly share the face's integer radius, must repaint
  the image backdrop (not just the colour), and must only touch corners a
  child reaches.
- Whatever the mask overpaints, `restore_container_outline` must redraw at
  the SAME boundary (Inside at face radius).
- Extend the shape-dump scenes + guard tests with any new corner behavior.

Related (non-corner egui upgrade regressions — Resize ratchet, popup
force-close): see the `egui-paint-regressions` skill.
