# Corner-bleed playbook — the complete, repeatable cure

> Companion to `SKILL.md`. Read `SKILL.md` first for the layered corner system;
> this file is the **deep, no-hints-left-behind** treatment of *bleed* — a fill
> or child painting past a container's rounded arc into the corner notch. Corner
> bleed has eaten an enormous share of this project's time. The reason is almost
> always the same two mechanisms below. Learn them once and the class is closed.

---

## 0. The single sentence that prevents 90% of it

> **egui clamps a rect's corner radius to HALF its shorter side. So any *filled*
> shape you want rounded to radius `R` must be at least `2*R` tall AND `2*R`
> wide, or it will render a smaller arc than `R` and poke past whatever it was
> supposed to hide behind. When you can't guarantee that size, you do NOT round
> the rect — you follow the arc with 1px horizontally-inset bands.**

Everything else here is a corollary of that plus the u8-radius corollary (§1.2).

---

## 1. The two root-cause mechanisms

Corner bleed is never magic. It is always one (or both) of these.

### 1.1 The radius **height-clamp** (the big one — cost us the most time)

`egui::epaint::RectShape` stores `corner_radius` as four `u8`s, and the tessellator
**clamps each corner to `min(requested, width/2, height/2)`**. Consequence:

- A fill you *asked* to round to `R=15` but that is only `h=3.5px` tall renders a
  **`1.7px`** arc. That tiny arc sits *inside* the big `R=15` silhouette, so the
  square-ish remainder of the fill shows in the notch **outside** the intended arc.
- This is invisible in unit tests that inspect `RectShape.corner_radius` — the
  stored value is still `15`. **You must compare the *effective* radius**
  `min(sw, w/2, h/2)`, not the stored one. (This is exactly why an earlier guard
  passed while the screen still bled — see §4.)

**Where it bites in practice:** a DataGrid whose **column filters** raise the
header height, squeezing the last visible row into a **few-pixel sliver**. That
sliver's row/cell fills request `R=grid_radius` and get a ~2px arc → bleed. Any
"last partial row / thin band at a rounded edge" is a candidate.

### 1.2 The u8-radius **round-up** (the classic, already documented in SKILL.md)

egui ≥0.31 radii are `u8`; the old stroke idiom `rect.shrink(half) + (r-half) +
StrokeKind::Middle` needs *fractional* radii that `u8` cannot hold. Rounding UP
pushes a concentric **stroke** arc *outside* the face (dark corner arcs); rounding
DOWN leaves a mask sliver (light corner arcs). Fixed permanently by: concentric
strokes use `StrokeKind::Inside` at the **full** rect + the **integer** face
radius (commit `f64efa9`). **Never reintroduce `shrink(half)` strokes.**

---

## 2. The cure, by shape kind (decision table)

| You are drawing… | Correct technique | Never do |
|---|---|---|
| A **stroke** concentric with a rounded face | `rect_stroke(FULL rect, integer face radius, StrokeKind::Inside)` | `shrink(half)` + fractional radius + `Middle` |
| A **tall** opaque fill (≥ `2*R` on both axes) reaching the corner | `rect_filled(rect, R, …)` — radius won't clamp | assume it's fine without checking height |
| A **short / thin** opaque fill reaching the corner (last row, sliver, 1px band) | **arc-inset bands** (§3) — follow the curve per scanline | round it (clamps → bleed) OR inset the whole rect square (leaves a square notch — "got worse") |
| The container **background** frost | already banded in `draw_glass` (`paint.rs`, the `arc_inset` closure) | — |
| **Child content** (image/grid inside a rounded parent) | notch mask + restore outline (form-level) or GL rounded clip (`COBOLT_ROUNDED_CLIP=1`) for nested/translucent | flat-mask a nested/translucent corner |

The trap that cost the extra round-trip: for a short fill, **insetting the whole
rect by `R` and drawing it square is WRONG** — it removes the bleed but leaves an
ugly square gap that doesn't track the arc. Bands are the only correct answer.

---

## 3. The band technique (canonical implementation)

This is the exact, copy-me pattern. It is what `draw_glass` uses for the frost and
what the DataGrid now uses for its row/cell fills. `screen` is the rounded rect,
`grid_cr` its radius.

```rust
// Fill `r` with `color`, but where it reaches a BOTTOM corner, follow the arc
// with 1px horizontally-inset bands (a rounded rect can't — see §1.1).
let fill_confined = move |painter: &egui::Painter, r: Rect, color: Color32| {
    let c = r.intersect(screen);
    if c.width() <= 0.0 || c.height() <= 0.0 { return; }
    let eps = 0.5;
    let at_bottom = (c.max.y - screen.max.y).abs() < eps;
    let at_left   = at_bottom && (c.min.x - screen.min.x).abs() < eps;
    let at_right  = at_bottom && (c.max.x - screen.max.x).abs() < eps;
    if !at_left && !at_right {
        painter.rect_filled(c, 0.0, color); // straight edge between corners → square is correct
        return;
    }
    let r_arc = grid_cr;
    // Horizontal inset of the arc at vertical position y (MATCHES draw_glass, incl. the +0.5).
    let arc_inset = |y: f32| -> f32 {
        let dy = (screen.max.y - y).abs();
        if dy >= r_arc || r_arc < 0.5 { 0.0 }
        else { (r_arc - (r_arc*r_arc - (r_arc-dy)*(r_arc-dy)).max(0.0).sqrt() + 0.5).max(0.0) }
    };
    // Part ABOVE the corner arc zone: one plain full-width rect.
    let zone_top = (screen.max.y - r_arc).max(c.min.y);
    if zone_top > c.min.y + eps {
        painter.rect_filled(Rect::from_min_max(c.min, pos2(c.max.x, zone_top)), 0.0, color);
    }
    // Corner zone: 1px bands, each inset by the arc at the band BOTTOM (widest → no bleed).
    let mut y = zone_top.max(c.min.y);
    while y < c.max.y {
        let yb = (y + 1.0).min(c.max.y);
        let inset = arc_inset(yb);
        let bx0 = if at_left  { c.min.x + inset } else { c.min.x };
        let bx1 = if at_right { c.max.x - inset } else { c.max.x };
        if bx1 > bx0 {
            painter.rect_filled(Rect::from_min_max(pos2(bx0, y), pos2(bx1, yb)), 0.0, color);
        }
        y = yb;
    }
};
```

Key correctness points, each learned the hard way:

- **Inset at the band BOTTOM** (`arc_inset(yb)`), the widest point of the band, so
  the fill never crosses the arc anywhere in the band. Over-inset is < 1px and
  invisible; under-inset bleeds. (`draw_glass` uses `max(top, bottom)` for the
  same reason.)
- **The `+0.5`** in `arc_inset` must match `draw_glass`, so the opaque fill lines
  up pixel-exactly with the frost bands beneath it. Drop it and you get a 0.5px
  seam between fill and frost.
- **Above the arc zone stays one rect** — don't band the whole fill (perf + the
  straight region has no arc to track).
- Only the corner(s) the fill actually reaches get inset (`at_left`/`at_right`);
  a middle column's cell must not be inset.

Generalizing to TOP corners: swap the edge in `arc_inset` from `screen.max.y` to
`screen.min.y` and gate on `at_top`. Same shape.

---

## 4. The repeatable DIAGNOSIS pattern (do THIS, in order)

The corner system is layered and pixel-exact; guessing moves the bug. Follow the
sequence — it is what actually localizes bleed every time.

1. **Pin the surface + style.** Designer (opaque theme hides half-pixel errors)
   vs Preview / **Run Form** (translucent viewport — artifacts show). Classic /
   Enhanced / Neumorphic exercise different layers. Bleed you can only see over a
   translucent backdrop is the flat-mask-can't-composite case (§5).

2. **Reproduce from the REAL form, not a guess.** The exact params matter (radius,
   row height, header height from filters, scroll → which row is the sliver). Pull
   them from the operator's `.cfrm` (`grep` the control's `<Property>`s) and build
   a shape-dump scene with those literal values. A scaled-down guess will pass
   while the real form bleeds.

3. **Shape-dump with EFFECTIVE radius.** Render the scene, walk every
   `egui::Shape::Rect`, and flag fills that **reach the corner** with
   `min(sw, w/2, h/2) < grid_radius - ε`. This catches the height-clamp (§1.1)
   that inspecting the *stored* radius misses. The permanent guard is
   `render.rs::shape_dump::datagrid_bottom_left_corner_has_no_opaque_bleed` —
   copy it for any new rounded-fill surface. It also asserts a point **inside**
   the arc IS filled, catching the "square gap / over-inset" regression.

4. **`COBOLT_FRAME_DIAGNOSTICS=1`** labels container corner painters
   (`CONTAINER_NOTCH_MASK`, `CONTAINER_RESTORE_OUTLINE`, `ROUNDCLIP_*`) on screen;
   the label at the artifact names the offending layer. NOTE: leaf controls drawn
   directly (DataGrid) are **not** instrumented by `debug_frame`, so this won't
   label a datagrid's own fills — use the shape-dump for those. (There is also a
   separate `COBOLT_DATAGRID_DIAGNOSTICS` overlay that outlines grid
   sub-components; it *reveals* bleed but doesn't fix or label the layer.)

5. **Diff dumps against a clean commit** (throwaway `git worktree`) when a bump
   regressed something — pre-tessellation radii/rects/clips compare directly.

### The effective-radius scan (copy-me)

```rust
let eff_sw = (rs.corner_radius.sw as f32).min(rs.rect.width()*0.5).min(rs.rect.height()*0.5);
let reaches_corner = rs.rect.min.x <= sx0 + 1.0 && rs.rect.max.y >= sy1 - 1.0 && rs.rect.min.y < sy1 - 1.0;
if reaches_corner && rs.fill.a() > 40 && !is_backdrop(rs.rect) && eff_sw < grid_r - 1.5 {
    /* BLEEDER — print bbox, fill, stored sw, eff_sw */
}
```

---

## 5. When the flat approach genuinely can't win

Over a **translucent** backdrop, or for a **nested** container, a flat notch mask
can't reproduce the see-through corner (it would repaint the *form* backdrop, not
the parent/wallpaper). Symptoms and the ONLY correct fixes:

- Bleed over translucent surface → **GL rounded clip** (`COBOLT_ROUNDED_CLIP=1`,
  `cobolt-ide/src/panels/rounded_clip.rs`): capture the real framebuffer, re-blit
  through an arc mask. Currently covers **GroupBox/Panel in the designer only** —
  NOT DataGrid, NOT Run Form. Extending it there is the real (sizable) fix if a
  leaf/grid must composite its corner over a translucent parent.
- A rounded control nested in a translucent parent: its corner notch **correctly
  reveals the parent** (that dark/tinted reveal is not a bug — verify with the
  shape-dump that only the *parent's* fills, not the child's, reach the notch).

Do not "fix" this by squaring the child or flat-masking — you'll trade a bleed for
a hole punched through the parent.

---

## 6. Invariants — obey when writing ANY corner code

- One radius source of truth per container: `paint::corner_radius(ctrl)`.
- Strokes concentric with a face: full rect + integer face radius + `Inside`.
  Never derive a fractional radius; `u8` can't hold it (§1.2).
- **Before rounding a filled rect, check it is ≥ `2*R` on both axes.** If not, use
  bands (§3). This is the rule that was missing for months.
- Band insets use the arc value at the band's widest edge, and MUST match
  `draw_glass`'s `arc_inset` formula (incl. `+0.5`) so opaque fills align with the
  frost.
- The notch mask must share the face's integer radius, repaint the image backdrop
  (not just colour), and only touch corners a child reaches. Whatever it
  overpaints, `restore_container_outline` redraws at the SAME boundary.
- **Every new rounded surface gets a shape-dump guard** asserting BOTH: no fill
  reaches the corner with effective radius < face radius (no bleed) AND a point
  inside the arc IS filled (no square gap).

---

## 7. Commit / code map

Historical fixes (do not re-derive these — they are shipped and pinned by tests):

- `f64efa9` — dark corner arcs on rounded panels (u8 radius round-up) → `Inside`
  strokes at integer face radius.
- `3a423af` — egui 0.31 `Rounding` → `CornerRadius` (the u8 transition that
  introduced the class).
- `bab42e1`, `40fcf82` — egui-035 WIP + merge: corner-bleed + run-form fixes.
- `5409dc0`, `fa0aa46` — the post-mortem skills (`egui-paint-regressions`, then
  this dedicated `rounded-corners` skill split out).

Current DataGrid short-fill fix (this playbook's subject) — commit `dfb4de2`
(1.34.3):

- `crates/cobolt-forms/src/render.rs` — `fill_confined` band helper (replaces the
  old `confine_bottom` rounded-rect) + its three call sites (alt-row, alt-column,
  cell background) in the `CT::DataGrid` arm.
- `crates/cobolt-forms/src/render.rs` — guard
  `shape_dump::datagrid_bottom_left_corner_has_no_opaque_bleed` (effective-radius
  + no-gap).
- `crates/cobolt-forms/src/paint.rs` — `draw_glass`'s `arc_inset` is the reference
  the band helper mirrors.

---

## 8. If it happens again — the 6-step drill

1. Which surface/style? (§4.1) Reproduce from the **real** `.cfrm` params (§4.2).
2. Shape-dump with **effective** radius; find the bleeder (§4.3, §3-scan).
3. Is it a fill too short/thin to hold its radius? → **bands** (§3). Is it a
   stroke? → `Inside` at integer face radius (§1.2). Is it translucent/nested? →
   GL clip or accept the parent-reveal (§5).
4. Add/extend a shape-dump guard: no bleed **and** no gap (§4, §6).
5. `cargo test -p cobolt-forms --features render --lib <guard>` red→green.
6. Bump `version.rs` fix number; note the commit id back into §7.
