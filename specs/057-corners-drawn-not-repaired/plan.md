# Plan — Spec 057: corners are drawn rounded, not repaired

- **Status:** approved (operator, 2026-09-05)
- **Spec:** specs/057-corners-drawn-not-repaired/spec.md

> Transcribed from the approved harness plan. R7 measurement tables are appended
> below as they are produced (baseline before any change, final after S4).

## Context

A rounded Panel/GroupBox's corners are painted twice. Visual children get a
transient `_ContainerClip` (`render.rs:2102` run, `:2603` faces; gate
`clips_to_container_border` `render.rs:644-652`) and draw their frame lifted to
the parent's arc (`control_border_rounding` `paint.rs:11436`, used at `:4095`).
Then `corner_notch_rounding` (`render.rs:779-823`) masks any corner whose
square *intersects* a descendant's rect (`:790-800`) — never asking whether
that descendant already clipped itself — and `draw_container_notch_mask`
(`paint.rs:11188`) repaints the **form backdrop** there. In `inner-form2` the
panel sits over two PictureBoxes, so the form's `rio0.png` is painted at the
wrong scale into every corner: the wedge chased for months as a transparency
bug. `restore_container_outline` and the offscreen GL clip only repair that
damage — and the GL clip is `egui_glow`-only on a **wgpu** build
(eframe-0.36 `default = […"wgpu"…]`, no `glow`; `egui-wgpu` logs
`Unknown paint callback` for it), so it has never run.

Outcome: a corner whose overlapping descendants all draw their own frame to
the arc is never repainted; every frame painter uses the control's resolved
radius; the dead GL path is gone.

## Decisions taken (operator, 2026-09-05)

- **Reproduction:** commit the operator's edited `inner-form2.cfrm` (Panel-1
  at 440,792, `Transparency=100`) — currently an uncommitted edit in the main
  checkout — as its own commit, BEFORE the fix. AC1 is built against it.
- **GL module:** delete `rounded_clip.rs`, its switch, the designer gate and
  the `render_faces` hook. CHANGELOG keeps the history.
- Fix-level bump: `version.rs` `1.65.1 → 1.65.2`; `fixes` branch.

## Design

**The rule, as one new function beside the guardian** (`render.rs`), leaving
`corner_notch_rounding` untouched so the existing guardian tests stay green
unmodified:

- `self_clipping_type(&ControlType) -> bool` — an **explicit allow-list**
  produced by the R7 measurement (below), not `clips_to_container_border`.
  `Custom{..}` excluded (`model.rs:2487-2488`).
- `descendant_clips_itself(container, child, neumorphic) -> bool` =
  `child.parent == container.id` (the seeding condition, `render.rs:670`,
  immediate parent only) `&& self_clipping_type(child)`
  `&& clips_to_container_border(child)` (`:644`)
  `&& child's drop shadow is not outward` (`paint::drop_shadow_spec`
  `:9528` / `is_overlay` `:9481` — an outward shadow at a corner spills into
  the notch; the shadow-on rows of the measurement decide whether this term
  can be dropped).
- `corners_already_correct(rect, radius, controls, idx, control_rects,
  neumorphic) -> [bool; 4]` — same four squares as `render.rs:799-821`; a
  corner is correct iff ≥1 descendant overlaps it (R4: guardian still decides
  "reached") AND every overlapping descendant `descendant_clips_itself`. A
  grandchild overlapping the corner ⇒ not correct (R5).
- `notch_mask_rounding` (`:760`) gains `neumorphic: bool`; in the
  GroupBox/Panel arm (`:773-774`) take the guardian rounding, zero each
  corner flagged correct, return `None` when all zero. Both painters already
  early-out on zero (`paint.rs:11198-11204`, `11321-11328`); the `None` only
  skips the shadow-stack build (`render.rs:874`) and the designer grid pass.
- `paint::neumorphic_active(ctx)` wraps the expression at `paint.rs:1432`.
- Call sites: `mask_container_notches` `render.rs:862-866`; the designer loop
  `designer.rs:7267-7273` (also pass `rounding` to
  `draw_grid_in_rounded_notches` at `:7309-7318`, and iterate
  `controls_for_render` `:7228-7234` — the list `control_rects` came from —
  instead of `self.form.controls` at `:7259/:7268`); unit callers
  `render.rs:9436-9470` append `false`.

Why the allow-list is measured, not read: `control_border_rounding` has two
callers (`paint.rs:4095`, `:7559`); every `draw_control_body` branch that
returns before `:4095` paints its frame **unlifted** — Slider (`:2694`, radius
`:2701`, selection literal `3.0` at `:3182-3187`), Knob/Gauge/Switch/
FileDropZone (`:3209`, radius `:3219`, border `:3670`), Maps (`:3750`,
`:3764/:3773`), ProgressBar (`:3860`, `:3865/:3944-3945/:3960-3972/:4000`).
inner-form2's corner children are three **Gauges**, so those branches must be
lifted or the allow-list cannot include them and AC1 cannot pass.

**Fallback if a type AC1 needs cannot be lifted** (optional S2b): refine
`hit` at `render.rs:800` to "the child rect contains a point of the square
farther than `r` from the arc centre" (for an axis-aligned rect: test its
corner nearest the container corner). Type-free, narrows only, keeps both
guardian tests green (the PictureBox at `:9484-9486` has (0,150) at 28.3px >
20 from (20,130)). Defer unless S4 forces it.

## Steps, in order

- **S0 — goldens first (AC4).** New `tests/mask_corner_goldens.rs` +
  `tests/goldens/057_*.txt`. Scenes that STILL mask after the fix: rounded
  Panel (r=24, shadow on, Fixed3D) → square child Panel (`HideBackground`,
  BorderStyle None) → Label with BackgroundColor covering all corners.
  Surfaces: run window (backdrop colour+gradient+image
  `TextureId::Managed(1)` as `tests/test_backdrop_image_modes.rs:18-30`);
  pane occupant (`color_hex "#00000000"`, `transparency 100`,
  `behind_fill: Some` as `render.rs:18009-18024`); `render_faces` + a replica
  of the designer loop (`designer.rs:7259-7308`). Golden = every non-Text
  `ClippedShape` intersecting a corner square, emission order, `dump_shape`
  format (`render.rs:16493-16558`) + FNV-64 of `format!("{:?}")`.
  `COBOLT_WRITE_GOLDEN=1` writes; else compare. Commit from the unchanged tree.
- **S1 — commit the reproduction form**, then **write AC1 red**: new
  `tests/inner_form2_corners_are_drawn_not_repaired.rs` — `load_form`
  (`xml.rs:451`) on `examples/PowerDemo3/forms/General/inner-form2.cfrm` via
  `CARGO_MANIFEST_DIR` (as `tests/test_maps_demo_form.rs:34`);
  `assets::set_base(examples/PowerDemo3)` so `rio0.png` and the PictureBox
  textures load; backdrop from `Form` fields (`model.rs:6662-6673`),
  `window_size` large enough that Panel-1's bottom corners are on screen.
  Assert per corner square, all three surfaces: no `Shape::Mesh` with all
  vertices inside the corner squares (notch fan/rings — `push_notch_fan`
  emits nothing for r<0.5, `paint.rs:10987-10989`, so this is a reliable
  signature), no stroked `Shape::Rect` whose clip rect is an r×r corner
  square (restore), and at a sample point in the notch the last covering
  shape is the PictureBox. **Re-derive which corners fail on the committed
  (edited) form at implementation** and record them; it must fail there.
- **S2 — R7 baseline measurement** on the unchanged tree (harness below);
  paste the table into `plan.md` as "baseline" (R14 evidence).
- **S3 — the rule** (Design above). AC1 passes for whichever types the
  baseline already marks self-clipping.
- **S4 — lift every frame painter (R10/R11):** `paint.rs:4260` selection
  stroke → `rect_stroke(frame_rect, frame_round, …)` (the sibling branch's
  rect/rounding, `:4242-4245`); Slider selection `3.0` → its lifted rounding;
  at the top of each early-return branch compute
  `let lifted = control_border_rounding(ctrl, rect, themed_corner_radius(ctx, ctrl))`
  and use it where the uniform value is used today (`paint_background_gradient`
  already takes a `CornerRadius`). **Shadow (E13):** `RegularDropShadow.corner_radius: f32`
  (`:9452`) → `egui::epaint::CornerRadiusF32` (`From<f32>`; per-corner round
  on conversion is identical to today for uniform values, so unclipped
  controls — the mask's only targets — stay byte-identical). In
  `regular_drop_shadow` (`:9517-9526`) set it from
  `container_image_rounding(rect ∩ border, border, rad, flags, own)` when
  `parse_container_clip(ctrl)` is `Some` (`:10663`), else `own`;
  `regular_shadow_stack` (`:9810`, `:9831`) adds `expand` per corner;
  `debug_frame` at `:2255` uses the same value. `control_shadow_stack`
  (`:1445`) inherits it — painter and sampler stay ONE definition (skill
  invariant). `drop_shadow_corner_radius` (`:9434`) unchanged.
- **S5 — re-measure (R7)**, fill `self_clipping_type`, paste the final table
  into `plan.md`; the harness asserts `{measured self-clips} ==
  {allow-list}` so neither can rot.
- **S6 — delete the GL path:** `panels/rounded_clip.rs`; `panels/mod.rs:37`;
  `designer.rs:7177-7191` (hook), `:7247` (drop the argument), `:7251-7257`
  (gate — loop always runs), `:7324-7325`; `debug_settings.rs:47-48, :84,
  :127, :141, :232-240` (old `debug_settings.toml` with `rounded_clip = true`
  still loads: `#[serde(default)]` `:40`, no `deny_unknown_fields`);
  `cobolt-ide/Cargo.toml:44-47` (`egui_glow`, its only user);
  `render.rs:320-336` (`RoundedClipHook` trait), `:2543` (param),
  `:2623-2644` (hook calls); the five `render_faces(` callers
  (`designer.rs:7247`, `render.rs:12659`, `tests/gauge_surfaces.rs:66`,
  `tests/non_visual_controls_stay_in_the_designer.rs:57`,
  `tests/test_radio_designer_run_parity.rs:98`).
- **S7 — adapt tests built on the old rule:**
  `notch_ambient_tests::a_corner_notch_ignores_the_ambient_panel_fill`
  (`render.rs:18084`, scene `:17986-17989`) goes RED — give it a grandchild
  (Label inside a `HideBackground`/BorderStyle-None child Panel) so the
  corner still masks; same scene change in
  `tests/a_faded_container_restores_a_faded_rim.rs:32-35` so it keeps
  exercising the restore. Add AC3/AC6/AC7/AC10 tests (below).
- **S8 — docs + release:** skill files, `parse_container_clip` doc
  (`paint.rs:10661` says "content rect"; the producer writes the BORDER rect,
  `render.rs:654-656`), CHANGELOG, version. Push `main` + `fixes`.

Full runs after S3, S4, S6: `cargo test -p cobolt-forms --features render`
and `cargo test -p cobolt-ide --bin cobolt-ide`.

## R7 measurement harness

New `tests/a_child_at_a_rounded_corner_stays_inside_the_arc.rs`. Per
`ControlType::ALL` entry (43, `model.rs:2489-2532`): outer Panel `OUT`
0,0 600×400 r=0 `HideBackground`, shadow off → inner Panel `IN` 40,40 400×300
r=40, shadow off, white → child `C` of type T, parent `IN`, rect (44,44,160,120)
straddling `IN`'s NW notch (4px inside the border, 50.9px from the arc centre).
Nested so the mask is off **by construction**: `notch_mask_rounding` returns
`None` for `IN` (parent set, `render.rs:768`) and ZERO for `OUT` (r=0), while
`_ContainerClip` is still seeded on `C` from `IN` (`:670-679`). Variants:
shadow off/on (SouthEast 7, blur 8), glass Classic and Neumorphic
(`paint::set_glass_style`, `render.rs:9500`). Surfaces per
`tests/gauge_surfaces.rs:41-72`: canvas `render_faces`, preview
`render_form(Interactive, DesignedState)`, run `render_form(Interactive,
Stringified)`. Isolate `C`'s shapes as `B.shapes[A.shapes.len()..]` (render
A = OUT+IN, B = OUT+IN+C; parent-before-child order, no post-passes).

Sample ≈340 pixel centres inside `IN`'s NW square and >40.75px from the arc
centre (0.75px AA margin). "Painted at p" per `ClippedShape` (clip_rect must
contain p): `Rect` — rounded-box SDF using each corner's **effective** radius
`min(stored, w/2, h/2)` (playbook §1.1), fill if `d≤0 && a>0`, stroke if
`|d−k|≤w/2`; `Mesh` — point-in-triangle over `indices.chunks_exact(3)` (as
`render.rs:18044`), textured meshes count and are flagged; `Path` even-odd /
segment distance; `Circle`/`Ellipse` normalised distance; `LineSegment`
distance; `Text` bbox (flagged, conservative); beziers/`Callback` bbox
(flagged); `Vec` recurse; `Noop` never. Output one line per
(type, surface, shadow, style): `R7 {type} {surface} shadow= style= bleed_px=
flags= shapes=`. Verdict: **self-clips** (bleed 0 everywhere), **paints
nothing** (0 shapes everywhere — non-visual at run, `render.rs:8892`), else
**bleeds**. The test prints the table and asserts the self-clips set equals
`self_clipping_type`.

## Tests → acceptance criteria

| AC | Test |
|---|---|
| AC1 | `inner_form2_corners_are_drawn_not_repaired.rs` (S1) — red before S3, green after; failing corners recorded |
| AC2 | `corner_notch_guardian_*` (`render.rs:9395`, `:9474`) — unmodified |
| AC3 | pure test beside `:9474`: PictureBox child at SW + Label grandchild at NE → only NE masked; both kinds on one corner → masked |
| AC4 | S0 goldens, three surfaces, unchanged through S3–S6 |
| AC5 | the R7 harness; baseline + final tables in `plan.md` |
| AC6 | one test in the AC1 file: three surfaces, identical per-corner decision |
| AC7 | `draw_control` direct (`tests/the_bars_carry_a_corner_radius.rs:50` pattern), `selected=true`, frameless rounded PictureBox: selection stroke radius == `frame_round`; child with shadow at a rounded parent corner via `render_form`: shadow rects carry the lifted radius on that corner |
| AC8 | `debug_settings` test: TOML with `rounded_clip = true` still loads (like `:580-585`); `grep -r "rounded_clip\|egui_glow\|COBOLT_ROUNDED_CLIP" crates/` empty |
| AC9 | `a_faded_container_restores_a_faded_rim` (adapted), `concentric_border_arcs_stay_inside_the_face` (`:17199`), `datagrid_bottom_left_corner_*` (`:17175/:17186`), `datagrid_fill_rects_*` (`:16937`), `datagrid_line_clip_keeps_lines_inside_the_arc` (`:9631`), `a_rounded_maps_corner_keeps_the_shadow_the_mask_paints_over` (`:17813`), `restore_outline_*` (`:9544/:9581`), `a_corner_notch_ignores_the_ambient_panel_fill` (adapted), `engine_reference_form_parity_static_vs_faces` (`:12605`) |
| AC10 | in `render.rs mod shape_dump` (reuse `composite_at` `:17631`, `painters_at` `:17580`): translucent Panel (40) with a self-clipping child over (i) form colour, (ii) form image, (iii) a PictureBox, (iv) a Label with BackgroundColor — no notch mesh/restore, and the notch pixel equals the same point rendered without the Panel |

## Risks and guards

- Self-clipping child + grandchild on one corner → `corners_already_correct`
  requires *every* overlapping descendant to clip; AC3 and the AC4 grandchild
  goldens pin it.
- Designer list vs render list — use `controls_for_render`; AC6 pins parity.
- Splitter panes: never masked today (`render.rs:768`; a pane is a Panel with
  a Splitter parent, `splitter.rs:434-444`) — unchanged. Stale pane clip via
  `plive.rect` (`render.rs:683` vs `resolved_rect` `:1500`) — follow-up.
- Child shadows/Neumorphic halos into the notch — the shadow term and the
  `neumorphic` flag; S5's shadow-on rows decide if they can be relaxed.
- Interactive egui widgets (TextEdit etc.) painting their own frames —
  measured on the run surface; bleeders simply stay masked.
- Non-visual descendants still trigger the mask via `control_rects`
  (`render.rs:2565`; badge on canvas, nothing at run) — unchanged, follow-up.
  Only 4 of 7 `is_non_visual()` types are in `clips_to_container_border`.
- Per-surface backdrop differences on corners that still mask (root pane
  `host.rs:3558-3562` `image: None`; occupant `host.rs:1620`
  `behind_fill: None`) — pre-existing; per-surface goldens make them visible.
- Shadow field type change ripples into the sampler the mask uses — the Maps
  shadow test (`:17813`) guards a wedge returning on masked corners.
- Designer selection border `designer.rs:7391-7404` uses an unlifted radius —
  editor chrome, not a frame painter; optional follow-up.
- Repeating-group instances: parent ids compared in the same expanded list the
  clip was seeded from (`render.rs:842-849`, `:670`) — agree by construction.

## Verification

1. Both suites green; AC1 demonstrably red before S3 and green after; the
   R7 equality assertion holds.
2. R7 tables (baseline, final) present in `plan.md`; every type the final table
   marks self-clipping is exactly the allow-list.
3. `rcrun build examples/PowerDemo3/PowerDemo3.project.toml`; operator opens
   `inner-form2` on designer / preview / run form — corners show the PictureBox
   behind the panel, no wedge (the agent does not drive the app).
4. `grep -rn "rounded_clip\|egui_glow\|COBOLT_ROUNDED_CLIP" crates/ docs/
   .claude/` — only CHANGELOG history remains; no `Unknown paint callback`
   warnings in a run.

## Docs to update (S8)

`.claude/skills/rounded-corners/SKILL.md`: line 3 (drop "GL clip"); item 6
`:39-43` (mask only where a descendant cannot clip itself; name
`corners_already_correct`); item 8 `:47-50` (remove; note it never ran —
glow-only on wgpu — removed in 1.65.2); table rows `:64-65`; step 2 `:74-75`
(drop `ROUNDCLIP_*`); invariants `:95-98` (never touch a corner whose
overlapping descendants self-clip; every frame painter, including
early-return branches, resolves its radius through `control_border_rounding`);
new failure-mode row "mismatched image wedge over a PictureBox / translucent
surface" naming the AC1 test. `CORNER-BLEED-PLAYBOOK.md`: row `:132`, §5
`:267-283` (no GL cure exists; the self-clip rule; nested = parent reveal; R7
harness for any new bleeder), §8 step 3 `:353-354`, §7 code map.

---

## Execution record

### T3 — AC1 pre-fix (unchanged tree, committed inner-form2)

Repaired corners of Panel-1, per surface, before any change:

| Surface | Repaired corners |
|---|---|
| Preview (`render_form`, DesignedState) | SE, SW |
| Run (`render_form`, Stringified) | SE, SW |
| Canvas (`render_faces` + `notch_mask_rounding`) | SE, SW |

Exactly the geometric prediction: `LineChart-1` (464,904 → 1008,1272)
intersects both bottom corner squares of Panel-1 (440,792 592×496, r=26);
no child reaches NW or NE. AC6 (three-surface agreement) is already green.

Harness note: `RawInput::max_texture_side` must be raised (the real
PictureBox image is 5225×2941; egui's headless default of 2048 trips a debug
assertion where a real backend would not). A GPU whose limit really is 2048
would refuse that image in the product too — outside this spec, recorded.

### T4 — R7 baseline (unchanged tree, 1.65.1)

`cargo test -p cobolt-forms --features render --test a_child_at_a_rounded_corner_stays_inside_the_arc -- --nocapture`, 43 types × 3 surfaces × {shadow off/on} × {Classic, Neumorphic}, bleed = pixel centres in the inner panel's NW corner square farther than 40.75 px from the arc centre that a child shape covers. Totals over the 12 variants:

| Type | bleed px (12 variants) | Verdict |
|---|---|---|
| AgentObject | 436 | bleeds |
| Animator | 856 | bleeds |
| AreaChart | 1308 | bleeds |
| BarChart | 1308 | bleeds |
| Button | 1311 | bleeds |
| CheckBox | 0 | self-clips |
| ComboBox | 1326 | bleeds |
| DataGrid | 1346 | bleeds |
| DateTimePicker | 1326 | bleeds |
| DonutChart | 1308 | bleeds |
| FileDropZone | 1361 | bleeds |
| Gauge | 549 | bleeds |
| GroupBox | 1326 | bleeds |
| IndexedFile | 436 | bleeds |
| Knob | 549 | bleeds |
| Label | 549 | bleeds |
| Line | 0 | self-clips |
| LineChart | 1308 | bleeds |
| ListBox | 1326 | bleeds |
| Maps | 1806 | bleeds |
| MenuBar | 879 | bleeds |
| NumericUpDown | 1326 | bleeds |
| Panel | 1326 | bleeds |
| PictureBox | 1326 | bleeds |
| PieChart | 1308 | bleeds |
| ProgressBar | 1527 | bleeds |
| RadioButton | 549 | bleeds |
| RestClient | 436 | bleeds |
| ScatterChart | 1308 | bleeds |
| Shape | 1896 | bleeds |
| SideMenu | 1227 | bleeds |
| Slider | 549 | bleeds |
| Snackbar | 540 | bleeds |
| Splitter | 1326 | bleeds |
| SqlDatabase | 436 | bleeds |
| StatusBar | 1402 | bleeds |
| Switch | 549 | bleeds |
| TabControl | 2139 | bleeds |
| TextBox | 1326 | bleeds |
| Timer | 436 | bleeds |
| ToolBar | 0 | self-clips |
| TreeView | 1326 | bleeds |
| WebSearch | 442 | bleeds |

Every type bled, for three reasons the dump (`COBOLT_R7_DUMP=<Type>`) separated: (1) `container_image_rounding` lifted a child's corner with a deliberately conservative *chord cut* (19 px where the concentric value is 36 for a 4 px inset under r=40), so every lifted frame poked 2–3 px past the arc — the 33 px shared by Panel, TextBox, PictureBox and the rest; (2) the drop shadow was never lifted at all (183 px on every type that has one, Classic only); (3) own-radius painters — Maps, Shape, ProgressBar, FileDropZone's dashes, the Snackbar pill, the interactive DataGrid/StatusBar/MenuBar/SideMenu/TabControl painters in render.rs. The non-visual types' 81 px is their designer badge.

### T7 — R7 final (this change)

Same harness plus a *dressed* variant (BackgroundColor, gradient, Single border) so a 0 cannot mean "paints nothing at that corner"; 24 variants per type. Worst bleed per surface:

| Type | Canvas | Preview | Run | Verdict |
|---|---|---|---|---|
| AgentObject | 0 | 0 | 0 | stays inside the arc |
| Animator | 0 | 0 | 0 | stays inside the arc |
| AreaChart | 0 | 0 | 0 | stays inside the arc |
| BarChart | 0 | 0 | 0 | stays inside the arc |
| Button | 0 | 0 | 0 | stays inside the arc |
| CheckBox | 0 | 0 | 0 | stays inside the arc |
| ComboBox | 0 | 0 | 0 | stays inside the arc |
| DataGrid | 0 | 96 | 96 | paints past the arc |
| DateTimePicker | 0 | 0 | 0 | stays inside the arc |
| DonutChart | 0 | 0 | 0 | stays inside the arc |
| FileDropZone | 0 | 133 | 133 | paints past the arc |
| Gauge | 0 | 0 | 0 | stays inside the arc |
| GroupBox | 0 | 0 | 0 | stays inside the arc |
| IndexedFile | 0 | 0 | 0 | stays inside the arc |
| Knob | 0 | 0 | 0 | stays inside the arc |
| Label | 0 | 0 | 0 | stays inside the arc |
| Line | 0 | 0 | 0 | stays inside the arc |
| LineChart | 0 | 0 | 0 | stays inside the arc |
| ListBox | 0 | 0 | 0 | stays inside the arc |
| Maps | 113 | 113 | 113 | paints past the arc |
| MenuBar | 0 | 0 | 0 | stays inside the arc |
| NumericUpDown | 0 | 0 | 0 | stays inside the arc |
| Panel | 0 | 0 | 0 | stays inside the arc |
| PictureBox | 0 | 0 | 0 | stays inside the arc |
| PieChart | 0 | 0 | 0 | stays inside the arc |
| ProgressBar | 0 | 0 | 0 | stays inside the arc |
| RadioButton | 0 | 0 | 0 | stays inside the arc |
| RestClient | 0 | 0 | 0 | stays inside the arc |
| ScatterChart | 0 | 0 | 0 | stays inside the arc |
| Shape | 0 | 0 | 0 | stays inside the arc |
| SideMenu | 0 | 0 | 0 | stays inside the arc |
| Slider | 0 | 0 | 0 | stays inside the arc |
| Snackbar | 0 | 0 | 0 | stays inside the arc |
| Splitter | 0 | 0 | 0 | stays inside the arc |
| SqlDatabase | 0 | 0 | 0 | stays inside the arc |
| StatusBar | 0 | 0 | 0 | stays inside the arc |
| Switch | 0 | 0 | 0 | stays inside the arc |
| TabControl | 303 | 303 | 303 | paints past the arc |
| TextBox | 0 | 0 | 0 | stays inside the arc |
| Timer | 0 | 0 | 0 | stays inside the arc |
| ToolBar | 0 | 93 | 93 | paints past the arc |
| TreeView | 0 | 0 | 0 | stays inside the arc |
| WebSearch | 0 | 0 | 0 | stays inside the arc |

`render::self_clipping_type` is the complement of the five that paint past the arc (DataGrid, FileDropZone, Maps, TabControl, ToolBar) and `Custom`; the harness asserts equality with the measurement, so neither can rot alone.

### T4–T11 — what the plan got wrong, and what was done about it

The plan (and the spec's R1) assumed a child that carries `_ContainerClip` is
drawn inside the arc, and that only the early-return painters were unlifted.
The R7 baseline said otherwise: **no type stayed inside the arc**, because the
lift itself — `container_image_rounding`'s "chord cut" — was deliberately
conservative (its own comment: "it will not erase pixels that are still inside
the parent's rounded border"), so every lifted frame poked 2–3 px past the arc,
and the shadow was never lifted at all. Reality diverged from the plan in five
places; each was measured before it was acted on (R14):

1. **The lift is concentric now.** `container_lift_radius` =
   `rad − max(inset_x, inset_y)`: for equal insets the container arc's
   concentric twin, contained by construction (`(max − min) + (rad − max) ≤
   rad`). This alone took the generic family — Panel, GroupBox, TextBox,
   ListBox, ComboBox, DateTimePicker, NumericUpDown, TreeView, Splitter,
   PictureBox, Label, Animator, the six charts — from 33 px to 0 with the
   shadow off. It changes how a PictureBox inside a rounded panel is cut
   (spec 017's behaviour): the corner is the parent arc's parallel, not a
   smaller arc that pokes out. The chord cut is gone.
2. **The clip is the CONTENT arc, not the border arc.** Children were clipped
   to the parent's content rect on their straight edges (`render.rs`, the
   `clip` from `containers::clip_rect`) but lifted to the BORDER arc at the
   corners — so a flush child covered the rim on the corners and nowhere
   else, and only the restore pass put it back. `container_clip_geometry`
   hands every child the content rect and `rad − inset`; the rule uses the
   same function. The stale comment "the clip widens to the border" described
   code that did not exist.
3. **Every type gets the clip.** The four non-visual types were excluded as
   "drawing nothing that can bleed" — true at run time, false on the canvas,
   where their badge drew square across the corner (81 px). `nv_card` takes
   the lift; the exclusion and `clips_to_container_border` are gone.
4. **The Neumorphic halo had two definitions.** `draw_glass_neumorphic`
   painted its own copy of the dual-halo loops, reachable only through
   `draw_surface_auto` with no control in hand — 193 px on nearly every type
   with the halo on. It now paints the ONE `neumorphic_shadow_stack`, and gets
   the clip from `ContainerClipScope`, a per-control scope `draw_control` (and
   the interactive MenuBar/StatusBar/DataGrid arms) publish in the egui
   context — the same mechanism the painter already used for
   `NeumorphicShadowParams`.
5. **Strips too short for their radius are rows.** The Button's top highlight
   (9 px tall) cannot hold a 30 px corner; `rows_inside_rounded_rect` lays it
   down trimmed to the face's arcs (the playbook's §1.1 rule, generalised).

Also lifted, as the harness named them: the shadow rings (`RegularDropShadow::
clip`, `lift_to_container` per ring, `DropShadowSpec::paint_in` for the
SideMenu rail), the selection outline (E12), the Slider (gradient, selection,
track halos), Knob/Gauge/Switch/FileDropZone (gradient; the drop zone's dashed
border runs along a per-corner outline), Maps (halo, gradient), ProgressBar
(trough, segments, border), Shape (face, gradient, outline), the SideMenu rail
(`SidebarState::clip`), and the interactive MenuBar/StatusBar/DataGrid faces.

**What still needs the mask, and why** (final table above): DataGrid — the
interactive grid's 22 px header band cannot hold the lifted radius and its
border line runs square; FileDropZone — the live egui widget paints its own
frame; Maps — square basemap tiles; TabControl — a tab is too small for the
radius its corner needs; ToolBar — with a colour or border the live bar paints
its own face. Each is a follow-up of its own; the mask keeps them right today,
and its output on those corners is byte-identical (T2 goldens).

**AC1** passes on all three surfaces with no type on the allow-list at all:
`inner-form2`'s SE/SW squares are touched only by a 2×10 px sliver of the
LineChart whose apex is 10.2 px from a 26 px arc's centre — inside the arc —
which `descendant_stays_inside_arc` reports geometrically. The Gauges the plan
named as the corner children touch no corner square.

**Tests moved by this change.** `a_corner_notch_ignores_the_ambient_panel_fill`
and `a_faded_container_restores_a_faded_rim`: their Label child now clips
itself, so the corner content is a grandchild inside a square holder panel
(the mask and the restore keep being exercised).
`picturebox_container_border_is_parent_visual_rect_and_radius` →
`…_is_parent_content_rect_and_concentric_radius`. The Elegance leaf-count
baseline moved by exactly +1 per row in both themes and all four styles: the
drop zone's dashed border is one pattern along its outline, 41 dashes where
four restarting edges made 40 (measured on a drop-zone-only scene).

**Not mine, seen in passing:** `assets::tests::a_relative_path_that_is_not_
under_the_anchor_keeps_the_old_behaviour` failed once in a full lib-test run
and passes alone and as a module — the asset base is process-global, so it
depends on test order. Pre-existing; not touched.
