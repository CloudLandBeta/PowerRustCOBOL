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

> ⚑ **For corner BLEED specifically (a fill/child painting past the arc), read
> [`CORNER-BLEED-PLAYBOOK.md`](CORNER-BLEED-PLAYBOOK.md) — the complete cure.**
> One rule closes ~90% of it: *egui clamps a rect's corner radius to half its
> shorter side, so a fill too short/thin to hold its radius (e.g. a partial last
> row) renders a smaller arc and bleeds. Follow the arc with 1px inset bands, not
> a rounded rect.* The playbook has the copy-me band code, the effective-radius
> diagnostic, the 6-step drill, and commit ids.

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
| **Opaque FILL bleeds past the arc** (partial last row / thin sliver / 1px band) | egui clamps a rect's `corner_radius` to `min(req, w/2, h/2)`; a short fill asked to round to `R` renders `height/2` — a tiny arc that pokes past the face. Guards that read the *stored* radius miss it (compare *effective* `min(sw,w/2,h/2)`) | Draw the fill as **1px arc-inset bands** (like `draw_glass`), never a rounded rect it's too short to hold, and never a square inset (leaves a notch). Full pattern + code: **`CORNER-BLEED-PLAYBOOK.md` §1.1/§3** |
| **Thin line at a FIXED y that flashes while scrolling** (a *gap*, not a bleed) | A decomposed fill (bands + plain strip) doesn't tile exactly — an `eps` threshold skipped a sub-pixel strip, so the layer beneath (the container's own BackgroundColor) shows through. Tell-tale: colour is a ~50% blend, line is pinned to a geometric boundary | Never guard a piece with `if height > eps`; make the decomposition tile exactly and unit-test it by sweeping fractional offsets. **`CORNER-BLEED-PLAYBOOK.md` §1.4** |
| **Thin DARK arcs** hugging corners (borders) | egui ≥0.31 radii are **u8**; the old idiom `rect.shrink(half) + (r−half) + StrokeKind::Middle` needs fractional radii; rounding UP pushes the stroke arc outside the face | Concentric strokes use `StrokeKind::Inside` at the full rect + integer face radius — exact, no fractional radius exists. Never reintroduce `shrink(half)` strokes |
| **Thin LIGHT arcs** at corners (mask sliver) | Same u8 problem, rounding DOWN: restored rim tighter than the mask boundary exposes masked backdrop | Same fix — Inside stroke outer edge == face edge == mask boundary |
| **Dark banding on corner diagonals** (neumorphic) | Shadow-fill radii (`r + fractional expand`) floored → every layer squarer | `round_map` uses round-to-nearest for FILLS (strokes don't go through it anymore) |
| **Flat WEDGE bitten out of the drop shadow at every corner** (obvious with a big `ShadowDistance`/`ShadowBlurStrength`) | The notch mask repaints the **form backdrop**, but the backdrop is not everything behind a control — its own shadow is painted there too and legitimately shows through the notch. The repaint erased it *inside* the bbox while the same shadow survived just outside, because the mask is clipped to the control's rect. That discontinuity is the wedge | `draw_container_notch_mask` takes the control's `ShadowStack` ([`paint::control_shadow_stack`]) and re-composites it over the repainted backdrop. Both shadow painters build layers through ONE definition so the sampler cannot drift; the notch is tessellated as a **radial grid** (`push_notch_rings`), because a fan samples a falloff only at the arc and the bbox corner. Use the alpha the control was DRAWN with. Pinned by `a_rounded_maps_corner_keeps_the_shadow_the_mask_paints_over` |
| **Thin dark HAIR tracing each corner arc, in the RUN form but NOT the designer** | `restore_container_outline` redrawing an outline the control's face never painted. It puts a rim + BorderColor border back on the masked corners; a control whose face returns before any border (Maps: halo → gradient → tiles → `return`) then gets an edge on the four arcs and nowhere else. The run-form-only tell is diagnostic: **designer.rs never calls restore at all** | Restore only for control types whose face draws an outline (`Panel`/`GroupBox`), stated positively so a new masked type opts in. Pinned by `restore_outline_skips_a_control_whose_face_draws_none`. Dump strokes, not fills — a hair is a stroke, and a notch/fill dump cannot see it |
| **Transparent/discoloured CRESCENT on a clean corner** | Notch mask painted over a corner **no child reaches**, destroying the container's own arc | `corner_notch_rounding` guardian: mask only corners a descendant overlaps. BOTH call sites must route through it — never call `draw_container_notch_mask` with blanket `CornerRadius::same(r)`. Pinned by `corner_notch_guardian_*` tests |
| **Backdrop-coloured hole punched through a parent panel** | Notch mask applied to a NESTED container repaints the *form* backdrop, not the parent surface | Nested containers skip the flat mask entirely; only the GL rounded clip can fix them properly |
| **Bleed visible over translucent surfaces** | Flat mask can't reproduce a see-through backdrop | GL capture/re-blit path (`COBOLT_ROUNDED_CLIP=1`) |
| **Child content square-clipped past the arc** (e.g. images) | egui rect-only clipping | Per-child `container_image_rounding` / `_ContainerClip` lifts the child's own corner radius where it lands on the parent arc (spec 017) |
| **BLACK/foreign-coloured WEDGES in many corners, only AFTER visiting another form — and permanently** | The notch fill came from the **ambient** `ui.visuals().panel_fill`: `notch_bg = composite(bg, panel_fill)`, and in **pane mode** the host hands the engine a *fully transparent* backdrop on purpose (the pane painted it), so `bg.a()==0` makes the ambient value **100 %** of the notch colour. No host fills its panel from the ambient visuals (Pane and see-through windows fill TRANSPARENT), so it was only ever a coincidence — and a **self-contained theme** (Elegance) writes its palette into the shared Context's GLOBAL style via `install_widget_visuals` → `Theme::install` → `global_style_mut`, with **no counterpart**: `LiquidGlassTheme::install_widget_visuals` is the trait-default no-op. So `panel_fill` stays `#0F172A` after that form is gone (operator, 2026-08-23) | The caller states what is behind it: `Backdrop::behind_fill: Option<Color32>`, set from `painted.bg` at the host's `Surface::Pane` branch; ambient stays only as the fallback for the designer canvas, which really does own its visuals. Pinned by `notch_ambient_tests::a_corner_notch_ignores_the_ambient_panel_fill` (renders the same scene under two ambient fills). **The theme-install asymmetry itself is still there** — it cannot be cleanly reversed, because `Theme::install` early-returns on a private ctx cache (`Id::new("elegance::theme")`), so restoring the baseline would leave the next Elegance form un-themed. Never resolve a paint colour from ambient visuals in the engine |

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
  the image backdrop (not just the colour), must re-composite the control's
  own **shadow** over what it repaints, and must only touch corners a child
  reaches.
- Anything that both PAINTS a stack of shapes and has to be SAMPLED later
  (shadows, gradients, relief) gets one definition that produces the layers,
  used by both. Deriving the geometry twice is how this codebase has
  repeatedly ended up with two painters that quietly disagree.
- Whatever the mask overpaints, `restore_container_outline` must redraw at
  the SAME boundary (Inside at face radius) — and **only what the face
  actually painted**. Restoring an outline a control never drew invents an
  edge on the corners alone.
- **Only the run form calls `restore_container_outline`** (`render.rs`); the
  designer canvas masks without restoring. So "artifact in the run form, clean
  in the RAD" points straight at the restore, while "both surfaces" points at
  the mask or the face. Ask the operator which surface, or read the two call
  sites — it is a two-grep answer.
- Extend the shape-dump scenes + guard tests with any new corner behavior.
  Dump **strokes as well as fills**: a hair is a stroke, and a fill/notch dump
  is blind to it.

Related (non-corner egui upgrade regressions — Resize ratchet, popup
force-close): see the `egui-paint-regressions` skill.
