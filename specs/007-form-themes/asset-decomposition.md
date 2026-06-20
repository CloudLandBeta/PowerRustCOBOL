# Addendum — Special-theme asset decomposition & reassembly contract

- **Spec:** ./spec.md   **Status:** spec only (no implementation)   **Date:** 2026-06-19

This addendum defines, for the **asset-pack ("special") themes** (starting with
`cobalt-steel`), exactly which PNG elements an external image agent must produce
and **how PowerRustCOBOL reassembles them onto a control** so the result matches
the high-resolution mockup at any control size. It also **specs the engine gaps**
(things 007's current single-image 9-slice cannot yet consume) — to be
implemented in a later phase, **not now**.

## 1. Scope: what a form theme skins

A form theme skins the **developer's form controls** (and an optional **form
background**). It does **not** skin the IDE's own window chrome, designer toolbar,
canvas grid, or IDE scrollbars — those in the mockup are **material reference
only** (use them to read the cobalt-steel finish; do not ship them as form-theme
assets). Themable controls are exactly the 20 the renderer recognises (§4).

## 2. How reassembly works today (the contract)

The renderer (`cobolt_forms::paint::draw_control`) draws a control by **9-slice**:
a source image is split into 4 fixed **corners**, 4 **edges** (stretched along
their run), and a **center** (stretched). The pack manifest gives the inset sizes
`slice = [left, top, right, bottom]`. Per-state images (normal/hover/pressed/
disabled/focused) swap the whole source. Text labels are drawn by code and are
**never** in the asset. An optional form **background** image and a chart
**palette + fill texture** are also consumed.

**Two equivalent delivery forms** — the agent may produce either; I can reassemble
both:

- **(A) Composite tile (consumable today):** one PNG per control+state at a
  reference size, authored continuously so corners sit within the insets and the
  center is stretch-safe. The engine slices it directly. **Zero seam risk** (one
  continuous render).
- **(B) Decomposed parts (the template's request; needs the parts-mode gap §5):**
  the 9 parts as separate files. I reassemble by placing corners fixed, stretching
  edges/center. **Parts must be cut from one continuous render** so they abut
  seamlessly; otherwise the brushed-steel grain shows seams.

> **Reassembly guarantee.** If you deliver a zip of decomposed parts (B) **plus**
> the per-control reference/inset metadata in §4, I can deterministically
> composite them into the engine's 9-slice source today, and consume them
> natively once the parts-mode (§5.1) lands. Corners are placed at exact inset
> size; edges/center stretch (or tile — §5.5). For pixel-faithful corners, the
> corner part dimensions **are** the insets.

## 3. Folder & naming convention

```
assets/themes/cobalt-steel/
  theme.toml                      # manifest (see §6)
  background.png                  # optional form background
  chart_fill.png                  # optional chart material fill (tileable)
  <control>/                      # one folder per control key (§4)
    <part>[_<state>].png
```

- `<control>` is one of the 20 keys in §4 (lowercase, exact).
- `<part>`: 9-slice parts `tl t tr l c r bl b br`; or named sub-parts/sprites/
  glyphs listed per control in §4.
- `<state>`: `normal hover pressed disabled focused` (+ `checked`/`selected` where
  §4 says so). `normal` may be omitted (it is the default/base).
- Export at **3×** logical size (filenames may carry `@3x`); ship `@1x` too if
  convenient. The engine downsamples.

Examples: `button/c_pressed.png`, `button/tl.png`, `slider/thumb_hover.png`,
`checkbox/box_normal.png`, `checkbox/check.png`, `combobox/button_normal.png`,
`combobox/arrow.png`.

## 4. Per-control asset manifest

Legend — **9S** = framed, needs the 9 parts (`tl t tr l c r bl b br`); **states**
= which state variants to produce (each multiplies the relevant parts/sprites);
**sub** = extra fixed sprites/glyphs beyond the frame. Ref = reference logical
size (authoring); inset = corner size (= the four 9-slice insets).

| Control key | Frame | States | Sub-parts (fixed sprites / glyphs) | Ref / inset |
|---|---|---|---|---|
| `button` | 9S | normal, hover, pressed, disabled, focused | — | 220×56 / 18 |
| `panel` | 9S | normal | — | 360×280 / 28 |
| `groupbox` | 9S | normal | `header_l header_c header_r` (title band, stretch) | 360×280 / 28 |
| `textbox` | 9S | normal, focused, disabled | — | 260×44 / 12 |
| `combobox` | 9S (field) | normal, focused, disabled | `button_{normal,hover,pressed}` (right, fixed 44w), `arrow` glyph | 260×44 / 12 |
| `listbox` | 9S | normal, focused, disabled | `row_selected` (1-px-tall stretch band, optional) | 280×200 / 14 |
| `datagrid` | 9S | normal | `header_l header_c header_r` (column header band), `gridline_v` `gridline_h` (1px tiles, optional) | 360×240 / 14 |
| `treeview` | 9S | normal | `expander_collapsed` `expander_expanded` glyphs (24×24) | 280×220 / 14 |
| `checkbox` | — (sprite) | box_{normal,hover,pressed,disabled,focused} | `check` glyph (drawn over box when checked) | box 28×28 |
| `radiobutton` | — (sprite) | knob_{normal,hover,pressed,disabled,focused} | `dot` glyph (checked) | knob 28×28 |
| `slider` | — (composite) | thumb_{normal,hover,pressed,disabled} | `track_l track_c track_r` (3-slice), `fill_l fill_c fill_r` (3-slice), `tick` (optional). Provide **horizontal**; note if vertical differs. | track 280×16 / cap 16; thumb 36×36 |
| `progressbar` | 9S (trough) | normal | `fill_l fill_c fill_r` (3-slice; `fill_c` tileable for animation) | 280×28 / 12 |
| `tabcontrol` | 9S (body) | normal | tab: `tab_l tab_c tab_r` × {selected, unselected} (3-slice tab, 120×40 / cap 16) | body 360×260 / 20 |
| `datetimepicker` | 9S (field) | normal, focused, disabled | `button_{normal,hover,pressed}` + `calendar` icon glyph | 220×44 / 12 |
| `numericupdown` | 9S (field) | normal, focused, disabled | `spin_up_{normal,hover,pressed}` `spin_down_{…}` (right, fixed 28w) + `arrow_up` `arrow_down` glyphs | 160×44 / 12 |
| `menubar` | 3S horizontal (`l c r`) | normal | `item_hover` (stretch band) | 600×40 / 16 |
| `toolbar` | 3S horizontal (`l c r`) | normal | `slot_{normal,hover,pressed,selected}` (48×48), `divider` (fixed) | 600×52 / 16 |
| `statusbar` | 3S horizontal (`l c r`) | normal | `grip` (fixed, bottom-right) | 600×32 / 12 |
| `splitter` | 3S along axis | normal, hover | `grip` (center dots). Provide horizontal + vertical. | 12×120 / cap 12 |
| `picturebox` | 9S (frame) | normal | — (only when ShowFrame; frameless = no asset) | 240×180 / 14 |
| charts (`barchart` `linechart` `piechart` `areachart` `scatterchart` `donutchart`) | 9S (frame, panel-like) | normal | shared `chart_fill.png` (tileable material for bars/slices); **data-mark colours come from the palette in theme.toml, not images** | 360×280 / 24 |

**Form background:** `background.png` — stretch-to-fill by default (`tile=false`),
or a seamless tile (`tile=true`). Optional `background_vignette.png` overlay.

**Excluded from all assets** (drawn/handled by code): text labels, captions,
runtime values, focus glow/animation, selection highlight, indicator LEDs, and
any application content. Provide `focused`/`hover`/`pressed` as **material**
states (e.g. a brighter bevel), not as animated light.

## 5. Engine gaps to implement later (spec only — DO NOT implement now)

The current engine consumes **(A)** single-image 9-slice + 5 states + background +
chart fill/palette. To consume the full decomposition above, these extensions are
required (future phase):

1. **Parts-mode 9-slice.** Accept a control skin defined by the 9 separate part
   files (`tl…br`) instead of one sliced image; corners fixed at their pixel size,
   edges/center stretched (or tiled, §5.5). Manifest: `mode = "parts"`.
2. **3-slice skins.** A horizontal/vertical 3-part frame (`l c r`) for menubar,
   toolbar, statusbar, splitter, slider track/fill, progressbar fill,
   tabcontrol tabs.
3. **Sprite controls.** `checkbox`/`radiobutton` render as a fixed-size **box/knob
   sprite + glyph overlay** (not a stretched frame), left-aligned, with the label
   drawn beside it. New skin kind `sprite`.
4. **Composite controls with sub-elements.** `slider` (track+fill+thumb+ticks),
   `progressbar` (trough+fill), `combobox`/`numericupdown`/`datetimepicker`
   (field + right-edge button(s) + glyph), `tabcontrol` (body + tab strip),
   `groupbox`/`datagrid` (header band). Each needs a small layout descriptor
   (where the sub-element sits, fixed width/height).
5. **Per-edge tile vs stretch + grain direction.** A flag per skin/edge
   (`tile_edges`, `tile_center`) and an optional `grain = "horizontal|vertical"`
   so brushed metal stretches along the grain and tiles across it without banding.
6. **Extra states.** Add `Checked`/`Selected` (checkbox/radio/tab/list row) to the
   `ControlState` set, plus glyph overlays (`check`, `dot`, `arrow_*`,
   `expander_*`, `calendar`, `sort_asc`/`sort_desc`).
7. **Reference/min size metadata.** Optional `reference_size` and `min_size` per
   skin so the renderer never shrinks a frame below its corner insets and can map
   the 3× authored art to logical pixels.
8. **Glyph/icon tinting hook** (optional): allow code to tint a glyph by the
   palette foreground so a single glyph serves light/dark variants.

Until these land, the **bridge path** is: composite the delivered parts into one
9-slice source tile per control+state at load (corners + stretched edges/center),
and feed the existing single-image path — visually identical for framed controls;
sprite/composite controls (checkbox, slider, combobox, …) fall back to Liquid
Glass until §5.3/§5.4 are implemented.

## 6. Manifest shape the agent should also emit

Alongside the PNGs, emit a `theme.toml` consistent with `theme_pack::ThemeManifest`
(extended fields from §5 are forward-looking and ignored by the current loader):

```toml
id = "cobalt-steel"
display_name = "Cobalt Steel"

[background]
image = "background.png"
tile = false

[palette]
foreground = "#dfe7ff"
chart = ["#5A82C8", "#C87A4C", "#4CC8A0", "#C84C82"]   # data-mark colours

[chart_style]
stroke_width = 2.5
fill_texture = "chart_fill.png"

# Framed controls — current single-image form (mode omitted) OR mode="parts".
[controls.button]
mode = "parts"                 # forward-looking (§5.1); omit for a composite tile
slice = [18, 18, 18, 18]
# state image refs (composite form) — or the parts live in button/ per §3
hover = "button/_hover.png"
pressed = "button/_pressed.png"
disabled = "button/_disabled.png"
focused = "button/_focused.png"
```

The authoritative, machine-checkable contract remains §3 (folders/names) + §4
(parts/states/insets). If a control in §4 has no art in the zip, it falls back to
Liquid Glass (R11) — partial packs are valid.
