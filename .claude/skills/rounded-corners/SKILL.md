---
name: rounded-corners
description: THE reference for PowerRustCOBOL's rounded-corner rendering — the layered corner system (face, concentric borders, children lifted to the arc, notch mask only where a child cannot clip itself, restore outline), every known recurring corner failure mode (dark/light corner arcs, crescents, bleed past the arc, erased rims, the mismatched image wedge) with root cause and exact fix, and the proven diagnosis workflow. Read BEFORE touching any code that paints, masks, clips, or strokes a rounded corner, whenever a screenshot shows artifacts at container corners, and before any egui version bump.
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
5. **Children draw their own frame to the arc** (spec 017, made true by spec
   057). egui clips to axis-aligned rects only, so every child of a rounded
   GroupBox/Panel is seeded a `_ContainerClip` — the parent's CONTENT rect and
   the concentric radius (`render::container_clip_geometry`), the same rect its
   straight edges are clipped to — and every frame painter resolves its radius
   through `paint::control_border_rounding` (`container_lift_radius`:
   `rad − max(inset_x, inset_y)`, the container arc's concentric twin) or, for
   a ring, `lift_to_container`: face, border, sheen and highlight rows, shadow
   rings, Neumorphic halo, non-visual badge, early-return branches (Slider,
   Knob/Gauge/Switch/FileDropZone, Maps, ProgressBar, Shape), the interactive
   MenuBar/StatusBar/DataGrid arms and the SideMenu rail. Which types actually
   stay inside the arc is **measured**, not read:
   `tests/a_child_at_a_rounded_corner_stays_inside_the_arc.rs` (R7 harness, 43
   types × 3 surfaces × 8 variants) asserts its result equals
   `render::self_clipping_type` — everything but DataGrid, FileDropZone, Maps,
   TabControl and ToolBar (each for a reason the harness prints).
6. **Corner-notch mask — only where a descendant cannot clip itself** —
   `notch_mask_rounding` = the guardian (`corner_notch_rounding`: which corners
   a descendant REACHES) minus `corners_already_correct` (which of those every
   reaching descendant stays inside on its own: an immediate child of a
   self-clipping type whose lifted radius fits it, or any descendant whose rect
   never leaves the arc). `mask_container_notches` → `draw_container_notch_mask`
   then repaints the form backdrop (colour AND background image) over the
   remaining notches. Form-level containers only; nested containers are skipped
   (their notches must reveal the parent surface, unknowable without
   compositing — and their children clip themselves anyway).
7. **Restore outline** — `restore_container_outline`: the mask overpaints the
   rim on the arcs it touched, so the rim (+ user border) is redrawn clipped to
   those corner squares. A self-clipping child never touches the rim (it is
   held inside the CONTENT arc), so nothing is restored on its corner.
8. ~~GL rounded clip~~ — **removed in 1.65.2 (spec 057).** It targeted the
   `egui_glow` backend, and the shipped build resolves eframe's default to
   **wgpu**, so it never ran once (egui-wgpu logged `Unknown paint callback`).
   Every "translucent surfaces need the GL path" note that survived was
   describing a cure that did not exist; the cure is item 5.

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
| **Backdrop-coloured hole punched through a parent panel** | Notch mask applied to a NESTED container repaints the *form* backdrop, not the parent surface | Nested containers skip the flat mask entirely; their children draw their own frame to the arc (item 5), which is why they need none |
| **Mismatched IMAGE WEDGE (a "triangle") in the corners of a translucent panel sitting over a PictureBox / another control — on designer, preview and run form alike** | The notch mask repaints the **form backdrop** into a corner, but the form backdrop is not what is behind that panel: a PictureBox is. So the form's own image appeared at the wrong scale in every corner (`inner-form2`, operator 2026-09-04/05). Chased for months as a transparency bug; every note pointed at the GL clip, which never ran (item 8). The mask ran at all only because the guardian asked "does a descendant intersect the corner square" — never "does it paint past the arc" — while the children (a LineChart sliver 10 px inside a 26 px arc; a chord-cut lift that let every child poke 2–3 px past the arc) were the real question | **Don't repair a corner that is drawn right.** `corners_already_correct` (beside the guardian, spec 057) zeroes any corner whose reaching descendants stay inside the arc; `container_lift_radius` lifts a child's corner to the container arc's concentric twin instead of the old conservative chord cut; the shadow rings, the Neumorphic halo and every early-return painter take the same lift. Pinned by `tests/inner_form2_corners_are_drawn_not_repaired.rs` (AC1, all three surfaces), `a_translucent_container_shows_what_is_behind_it.rs` (AC10) and the R7 harness |
| **Child content square-clipped past the arc** (e.g. images) | egui rect-only clipping | Per-child `_ContainerClip` (content rect + concentric radius) and `control_border_rounding` / `container_lift_radius` lift the child's own corner radius where it lands on the parent arc (spec 017; concentric since 057) |
| **BLACK/foreign-coloured WEDGES in many corners, only AFTER visiting another form — and permanently** | The notch fill came from the **ambient** `ui.visuals().panel_fill`: `notch_bg = composite(bg, panel_fill)`, and in **pane mode** the host hands the engine a *fully transparent* backdrop on purpose (the pane painted it), so `bg.a()==0` makes the ambient value **100 %** of the notch colour. No host fills its panel from the ambient visuals (Pane and see-through windows fill TRANSPARENT), so it was only ever a coincidence — and a **self-contained theme** (Elegance) writes its palette into the shared Context's GLOBAL style via `install_widget_visuals` → `Theme::install` → `global_style_mut`, with **no counterpart**: `LiquidGlassTheme::install_widget_visuals` is the trait-default no-op. So `panel_fill` stays `#0F172A` after that form is gone (operator, 2026-08-23) | The caller states what is behind it: `Backdrop::behind_fill: Option<Color32>`, set from `painted.bg` at the host's `Surface::Pane` branch; ambient stays only as the fallback for the designer canvas, which really does own its visuals. Pinned by `notch_ambient_tests::a_corner_notch_ignores_the_ambient_panel_fill` (renders the same scene under two ambient fills). **The theme-install asymmetry itself is still there** — it cannot be cleanly reversed, because `Theme::install` early-returns on a private ctx cache (`Id::new("elegance::theme")`), so restoring the baseline would leave the next Elegance form un-themed. Never resolve a paint colour from ambient visuals in the engine |

## Diagnosis workflow (what actually works — in this order)

1. **Ask which surface**: designer (opaque theme masks half-pixel errors) vs
   Preview/Run Form (transparent viewports — artifacts show). Which glass
   style — Classic/Enhanced/Neumorphic exercise different layers.
2. **`COBOLT_FRAME_DIAGNOSTICS=1`** — labels every corner painter on screen
   (`CONTAINER_SHADOW`, `CONTAINER_FACE`, `CONTAINER_NOTCH_MASK`,
   `CONTAINER_RESTORE_OUTLINE`); the label at the artifact names the offending
   layer. A `CONTAINER_NOTCH_MASK` on a corner whose only occupant is a child
   of a self-clipping type means the child's painter regressed: run the R7
   harness (`COBOLT_R7_DUMP=<Type>` prints the bleeding shapes).
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

- **Never repaint a corner whose reaching descendants stay inside the arc**
  (spec 057). The mask is a repair for content that *cannot* clip itself; on a
  corner that is already right it can only paint the wrong thing (the form
  backdrop where something else is behind). `corners_already_correct` is the
  one function that decides, beside the guardian, for all three surfaces.
- **Every frame painter resolves its radius through the container lift** —
  `control_border_rounding` for a rect, `lift_to_container` for a ring, the
  `ContainerClipScope` for a surface painter reached without the control in
  hand — including early-return branches, shadows, halos, badges, highlight
  strips and the interactive arms in `render.rs`. A new frame drawn square is
  a defect, not a style choice. Do not trust a painter you have not measured:
  the R7 harness renders every type at a corner and asserts the allow-list.
- **One clip geometry for all four sides**: the parent's CONTENT rect and the
  concentric radius (`container_clip_geometry`). Lifting to the BORDER arc
  while clipping the straight edges to the content rect is how a flush child
  covered the rim at the corners and nowhere else.
- A strip too short to hold the radius its corner needs (egui caps a radius at
  half the height) cannot be a rounded rect: lay it down as rows trimmed to
  the face's arcs (`rows_inside_rounded_rect`) — the same rule as the
  playbook's §1.1 bands.

Related (non-corner egui upgrade regressions — Resize ratchet, popup
force-close): see the `egui-paint-regressions` skill.
