# Handoff — egui 0.29→0.34 upgrade: rounded-corner transparency issue

## Current state

The egui upgrade from **0.29 to 0.34** is complete. The IDE compiles and runs
with 0 errors (205 deprecation warnings). All 44 cobolt-forms tests pass.
However, there is a **visual regression** at rounded container corners that was
not present in egui 0.29.

## The problem

**Thin transparent seam at rounded corners** of GroupBox/Panel containers. A
1–2px gap of transparency is visible at the corners where the container's
glass fill meets the corner notch mask. The form's background image/color
shows through this gap. The issue affects ALL rounded containers (GroupBox,
Panel) and all children inside them (PictureBox images especially visible).

### Screenshots

The artifact is a thin arc of background showing through at each rounded
corner of a container. It's most visible when:
- The container has a background image behind it (leather texture, etc.)
- A PictureBox image overflows the container bounds
- The container is nested inside another container

## Root cause

egui 0.34 changed `egui::Rounding` (f32 fields) to `egui::CornerRadius`
(u8 fields). This u8 quantization (0–255 integer pixels) creates a precision
mismatch between three systems that must agree pixel-perfectly:

1. **Glass fill** — stacked `rect_filled` bands with `CornerRadius` (u8)
2. **Notch mask** — custom triangle fan mesh using `corner_radius.nw as f32`
3. **egui's internal anti-aliasing** — feathers the edges of `rect_filled`
   by ~1px, but the notch fan has hard edges

In egui 0.29, all three used `f32` values and matched exactly. In 0.34:
- `rect_filled` uses u8 CornerRadius internally, adds ~1px AA feathering
- The notch fan uses `u8 as f32` (integer) for its arc — no feathering
- The feathered edge of the glass fill extends ~1px past the notch fan's arc
- Result: a thin transparent seam between the glass fill edge and the notch

## What has been tried

### Fixes applied (still in the code)

1. **`round_map` function** — converts `u8→f32→f32→u8` with `.round()` for
   all corner radius transformations.

2. **`container_image_rounding`** — uses `.round() as u8` consistently for
   container-clipped corners (was using `ceil` and `ceil+1` at various
   points, creating mismatches).

3. **Notch mask for nested containers** — previously, nested containers
   (GroupBox inside Panel) had their notch masks completely skipped (the
   code had `if ctrl.parent.is_some() { continue; }`). Now ALL containers
   get notch masks. Nested containers use the form backdrop as their notch
   fill (an approximation, since the true parent surface would need
   offscreen compositing).

4. **Notch fan overlap** — the notch fan radius is `corner_radius + 1px` so
   the fan slightly overlaps the anti-aliased edge of egui's rect_filled.
   This covers most of the feathering gap but not 100%.

5. **PictureBox glass card** — when a PictureBox has a loaded texture AND
   is inside a container, both the glass card (`frame_round`) and the image
   (`rounding`) use `container_image_rounding` with the same values, so they
   agree.

6. **PictureBox clipped edges** — `draw_media_image` applies
   `container_image_rounding` to the image at container clip boundaries
   (matching the pre-upgrade behavior). The image curves to match the
   container's arc at clip points.

### Fixes tried and reverted

- **Zeroing rounding on clipped edges** — set radius=0 on image corners
  where the container clips. This prevented forced rounded corners on
  clipped edges but caused the notch mask to not cover the square corners
  (the nested container notch was being skipped at the time).

- **Skipping glass card for PictureBox with texture** — set `pic_frameless`
  when a texture is loaded, preventing the glass card from being drawn behind
  the image. This eliminated the glass card corner peek-through but caused
  the image to bleed past the container with no covering.

- **`ceil()` vs `round()` for u8 conversion** — tried `ceil` to ensure the
  arc always curves MORE than the computed float value. This created
  mismatches with the notch mask (which used `round`) and made the gap
  worse.

- **Notch fan +2px overlap** — tried making the notch fan 2px larger. This
  covered the transparency gap but caused the notch to encroach into the
  container's content area, re-creating the forced corner artifact.

## Key files

| File | Role |
|------|------|
| `crates/cobolt-forms/src/paint.rs` | `draw_glass`, `draw_glass_enhanced`, `draw_media_image`, `container_image_rounding`, `control_border_rounding`, `notch_mesh`, `push_notch_fan`, `draw_container_notch_mask`, `round_map`, `composite_premultiplied_over` |
| `crates/cobolt-forms/src/render.rs` | `mask_container_notches` (runtime notch mask loop), `render_faces` (control draw loop), `container_clip_prop` / `parse_container_clip` |
| `crates/cobolt-ide/src/panels/designer.rs` | Designer canvas notch mask loop (line ~2312), `render_faces` call, notch fill/image computation |

## Key functions to understand

- **`draw_glass` / `draw_glass_enhanced`** — draws the frosted glass fill
  using stacked 1px `rect_filled` bands with arc-inset. Uses `rnd`
  (CornerRadius, u8).

- **`notch_mesh` / `push_notch_fan`** — draws triangle fans in each corner
  notch (the area between the square corner and the rounded arc). Uses
  `corner_radius.nw as f32` for the arc math.

- **`draw_container_notch_mask`** — called after all children are drawn.
  Paints the backdrop (solid fill + optional background image) in the
  corner notch areas to cover any child content that bled past the arc.

- **`container_image_rounding`** — computes per-corner radius for a child
  (PictureBox, Animator) inside a rounded container. Returns CornerRadius
  with the container's arc radius on corners where the child touches the
  container border.

- **`round_map`** — applies a function to each corner of a CornerRadius,
  converting `u8→f32→f32→u8` with `.round()`.

## Possible approaches for the next agent

1. **Match egui's internal feathering** — study how egui 0.34 tessellates
   rounded rects (the `RectShape` tessellator in `epaint`) and make the
   notch fan match its exact arc + feathering. The notch fan currently uses
   a simple triangle fan with N segments, while egui may use a different
   segment count or feathering algorithm.

2. **Use egui's own `rect_filled` for the notch** — instead of a custom
   triangle fan, draw the notch as four overlapping `rect_filled` calls
   positioned at each corner. Since `rect_filled` uses egui's own
   tessellation, the arcs would match exactly. The challenge: `rect_filled`
   fills the INSIDE of the rounded rect, but the notch needs the OUTSIDE.

3. **Draw the notch with `Shape::Callback`** — use egui's callback shape to
   do custom rendering that exactly matches the tessellation.

4. **Accept the 1px artifact** — the +1px notch overlap covers most of the
   gap. The remaining sub-pixel seam may be acceptable given egui 0.34's
   u8 limitation. Focus on other issues instead.

5. **Investigate `CornerRadius` as `f32`** — check if there's a way to pass
   f32 corner radii to egui 0.34's `rect_filled` (perhaps via `RectShape`
   directly with f32 `corner_radius` field, since the struct stores it as
   `CornerRadius` which is u8). If not, consider a PR to egui or a local
   fork.

## Other pending items

- The egui-elegance integration (spec 019) is blocked on this fix
- 205 deprecation warnings in the IDE should be cleaned up
- The `Multiline` TextBox property fix is done
- Menu editor improvements (accelerator capture, icon picker, form dropdown)
  are done
- 322 vector icons across 22 categories are implemented
