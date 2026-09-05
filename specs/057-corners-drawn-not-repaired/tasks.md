# Tasks — Corners are drawn rounded, not repaired

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-09-05

Ordered so the tree stays green after every task, with one stated exception:
**AC1 is written red in T3 and is allowed to stay red until T7**, because it
turns green only once the allow-list contains the types that sit in
`inner-form2`'s corners, and that list is a *measured* fact (R7), not one
written down in advance.

> **Classification: fix** (spec §6) — fix-number bump, `fixes` branch, never
> sharing a commit with a feature (GOLDEN RULE #5). Push per plan S8, which the
> operator approved.
>
> **Standing rule for every task:** no claim about a painter without a
> file:line or a rendered measurement (R14). The corner system has recurred
> precisely because one layer assumed another.

---

## Stage A — evidence before any change

- [x] **T1 — Commit the reproduction form** (plan S1, AC1 prerequisite)
  - Files: `examples/PowerDemo3/forms/General/inner-form2.cfrm`
  - Do: copy the operator's edited working copy (Panel-1 at 440,792 592×496,
        `Transparency=100`, `CornerRadius=26`) from the main checkout into this
        worktree; commit on its own (`PowerDemo3: inner-form2 as the corner
        reproduction`).
  - Verify: `git show HEAD:examples/PowerDemo3/forms/General/inner-form2.cfrm |
        grep -o '<Control id="Panel-1"[^>]*>'` shows `x="440" y="792"`; the
        panel's `Transparency` reads `100`.

- [x] **T2 — Goldens of the corners that will STILL mask** (plan S0, AC4)
  - Files: new `crates/cobolt-forms/tests/mask_corner_goldens.rs`; new
        `crates/cobolt-forms/tests/goldens/057_window.txt`, `057_pane.txt`,
        `057_faces.txt`
  - Do: scene = rounded Panel (r=24, `ShadowEnabled`, `BorderStyle=Fixed3D`) →
        square child Panel (`HideBackground`, `BorderStyle=None`) → Label with
        `BackgroundColor` covering all four corners (a **grandchild** keeps the
        mask under the new rule). Three surfaces exactly as plan S0: run window
        (colour + gradient + image `TextureId::Managed(1)`), pane occupant
        (`color_hex "#00000000"`, `transparency 100`, `behind_fill: Some`), and
        `render_faces` plus a replica of the designer loop
        (`designer.rs:7259-7308`). Golden = every non-Text `ClippedShape`
        intersecting a corner square, emission order, `dump_shape`-style line
        (`render.rs:16493`) plus an FNV-64 of `format!("{:?}")`.
        `COBOLT_WRITE_GOLDEN=1` writes, otherwise compares.
  - Verify: `COBOLT_WRITE_GOLDEN=1 cargo test -p cobolt-forms --features render
        --test mask_corner_goldens` writes three files from the **unchanged**
        tree; a second run without the env var is green; goldens committed
        before T5. **AC4 baseline captured.**

- [x] **T3 — AC1 and AC6, written red** (R1, R2, R8; AC1, AC6)
  - Files: new `crates/cobolt-forms/tests/inner_form2_corners_are_drawn_not_repaired.rs`
  - Do: `cobolt_forms::load_form` (`xml.rs:451`) on the committed form via
        `CARGO_MANIFEST_DIR` (pattern `tests/test_maps_demo_form.rs:34`);
        `assets::set_base(<repo>/examples/PowerDemo3)`; backdrop from the
        `Form` fields; `window_size` tall enough that Panel-1's bottom corners
        (y to 1288) are on screen. For Panel-1's four r×r corner squares, on
        `render_form(Interactive, DesignedState)`, `render_form(Interactive,
        Stringified)` and `render_faces` + `notch_mask_rounding`: assert **no**
        `Shape::Mesh` whose vertices all lie inside the corner squares (the
        notch fan — `push_notch_fan` emits nothing for r<0.5,
        `paint.rs:10987`), and **no** stroked `Shape::Rect` whose clip rect is a
        corner square (the restore, `paint.rs:11382`). AC6: one test asserting
        the three surfaces' per-corner decisions are identical.
  - Verify: `cargo test -p cobolt-forms --features render --test
        inner_form2_corners_are_drawn_not_repaired` **FAILS**. Geometry
        predicts SE and SW (`LineChart-1` 464,904→1008,1272 reaches both
        bottom squares; nothing reaches NW/NE). **Record the failing corners in
        plan.md** — if they differ from the prediction, the difference is a
        finding, not noise.

- [x] **T4 — R7 harness and the baseline table** (R7, R14; AC5 first half)
  - Files: new `crates/cobolt-forms/tests/a_child_at_a_rounded_corner_stays_inside_the_arc.rs`;
        `specs/057-corners-drawn-not-repaired/plan.md` (append "R7 — baseline")
  - Do: per `ControlType::ALL` entry (43, `model.rs:2489`): OUT Panel 0,0
        600×400 r=0 `HideBackground` → IN Panel 40,40 400×300 r=40 white → child
        of type T at (44,44,160,120) straddling IN's NW notch. Variants:
        shadow off/on, glass Classic/Neumorphic (`paint::set_glass_style`).
        Surfaces per `tests/gauge_surfaces.rs:41-72`: canvas `render_faces`,
        preview `render_form(DesignedState)`, run `render_form(Stringified)`.
        Isolate the child's shapes as `B.shapes[A.shapes.len()..]`. Sample
        ~340 pixel centres inside IN's NW square and >40.75 px from the arc
        centre; coverage per shape kind as plan §R7 (Rect via rounded-box SDF
        with **effective** radius; Mesh point-in-triangle; Path; Circle;
        LineSegment; Text/bezier/Callback by bbox, flagged). Print one line per
        (type, surface, shadow, style). Verdict per type: self-clips / paints
        nothing / bleeds. **No allow-list assertion yet.**
  - Verify: `cargo test -p cobolt-forms --features render --test
        a_child_at_a_rounded_corner_stays_inside_the_arc -- --nocapture` prints
        43 types × 3 surfaces × 4 variants; the table is pasted verbatim into
        plan.md as the baseline. Expected from plan S4: Slider, Knob, Gauge,
        Switch, FileDropZone, Maps, ProgressBar bleed (their branches return
        before `frame_round`, `paint.rs:4095`).

## Stage B — the cause

- [x] **T5 — The rule, beside the guardian** (R1, R2, R3, R4, R5, R8, R9)
  - Files: `crates/cobolt-forms/src/render.rs`; `crates/cobolt-forms/src/paint.rs`;
        `crates/cobolt-ide/src/panels/designer.rs`
  - Do: in render.rs add `self_clipping_type` (seeded with T4's self-clips set;
        `Custom{..}` excluded), `descendant_clips_itself` (immediate parent ==
        container `render.rs:670` && `self_clipping_type` &&
        `clips_to_container_border` `:644` && the child's drop shadow is not
        outward, `paint::drop_shadow_spec` `:9528`/`is_overlay` `:9481`), and
        `corners_already_correct` (same four squares as `:799-821`; correct iff
        ≥1 descendant overlaps AND every overlapping descendant clips itself).
        `notch_mask_rounding` gains `neumorphic: bool` and zeroes correct
        corners in the GroupBox/Panel arm, returning `None` when all zero.
        `corner_notch_rounding` **untouched**. paint.rs: `neumorphic_active(ctx)`
        wrapping the expression at `:1432`. Call sites: `mask_container_notches`
        `:862-866`; designer loop `:7267-7273` — also pass `rounding` (not
        `same(cr8(rad))`) to `draw_grid_in_rounded_notches` `:7309-7318`, and
        iterate `controls_for_render` `:7228-7234` instead of
        `self.form.controls` at `:7259/:7268`; unit callers `:9436-9470` append
        `false`. Add the AC3 pure tests beside `render.rs:9474`.
  - Verify: `cargo test -p cobolt-forms --features render` — `corner_notch_
        guardian_*` green **unmodified**; AC3: PictureBox child at SW + Label
        grandchild at NE → only NE masked; both kinds on one corner → masked.
        T2 goldens unchanged. AC1 green **only if** T4 marked LineChart
        self-clipping; otherwise still red — expected, say so.

- [x] **T6 — Every frame painter takes the lifted radius** (R10, R11; AC7)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: selection stroke `:4260` → `rect_stroke(frame_rect, frame_round, …)`
        (the sibling branch's rect/rounding `:4242-4245`); Slider selection
        literal `3.0` `:3182-3187` → the Slider's lifted rounding; at the top of
        each early-return branch — Slider `:2694`, Knob/Gauge/Switch/
        FileDropZone `:3209`, Maps `:3750`, ProgressBar `:3860` — compute
        `let lifted = control_border_rounding(ctrl, rect,
        themed_corner_radius(ctx, ctrl))` and use it where the uniform value is
        used (`:2701`, `:3219`, `:3670`, `:3764/:3773`, `:3865/:3944-3945/
        :3960-3972/:4000`). **Shadow:** `RegularDropShadow.corner_radius: f32`
        `:9452` → `egui::epaint::CornerRadiusF32`; in `regular_drop_shadow`
        `:9517-9526` set it from `container_image_rounding(rect ∩ border,
        border, rad, flags, own)` when `parse_container_clip(ctrl)` is `Some`,
        else `own`; `regular_shadow_stack` `:9810/:9831` adds `expand` per
        corner; `debug_frame` `:2255` uses the same value.
        `drop_shadow_corner_radius` `:9434` unchanged — painter and sampler
        (`control_shadow_stack` `:1445`) stay one definition.
  - Verify: AC7 tests — `draw_control` direct (`tests/the_bars_carry_a_corner_
        radius.rs:50` pattern), `selected=true`, frameless rounded PictureBox:
        selection stroke radius == `frame_round`; a child with `ShadowEnabled`
        at a rounded parent corner via `render_form`: its shadow rects carry
        the lifted radius on that corner. `a_rounded_maps_corner_keeps_the_
        shadow_the_mask_paints_over` (`render.rs:17813`) and
        `drop_shadow_corner_radius_matches_control_silhouette`
        (`paint.rs:14370`) green. **T2 goldens unchanged** (unclipped controls
        convert a uniform radius identically).

- [x] **T7 — Re-measure and finalise the allow-list** (R7; AC5, AC1)
  - Files: `crates/cobolt-forms/src/render.rs` (`self_clipping_type`); the T4
        harness (add the equality assertion); `plan.md` (append "R7 — final")
  - Do: rerun T4; every type that now self-clips on every surface and variant
        goes into `self_clipping_type`; the harness asserts
        `{measured self-clips} == {self_clipping_type}` so neither can rot; its
        doc comment cites the plan.md table. If a type `inner-form2` needs
        still bleeds and cannot be lifted, apply plan S2b (geometric `hit`
        refinement at `render.rs:800`) and record why.
  - Verify: harness green with the assertion; **AC1 green**, with the corners
        that flipped recorded in plan.md; AC6 green; the full
        `cargo test -p cobolt-forms --features render` green except the two
        tests T8 adapts.

## Stage C — everything that leaned on the old rule

- [x] **T8 — Adapt the tests that triggered the mask with an immediate child** (AC9)
  - Files: `crates/cobolt-forms/src/render.rs` (`notch_ambient_tests`, scene
        `:17986-17989`); `crates/cobolt-forms/tests/a_faded_container_restores_
        a_faded_rim.rs` (scene `:32-35`)
  - Do: give each scene a grandchild (Label inside a `HideBackground`/
        `BorderStyle=None` child Panel) so the corner still masks and the test
        still exercises what it was written for. Assertions unchanged.
  - Verify: `a_corner_notch_ignores_the_ambient_panel_fill` green;
        `a_faded_container_restores_a_faded_rim` green **and** its rim
        assertion still finds a restored stroke (it must not pass vacuously).

- [x] **T9 — AC10: no wedge over any backdrop** (AC10)
  - Files: `crates/cobolt-forms/src/render.rs` (`mod shape_dump`, reuse
        `composite_at` `:17631` and `painters_at` `:17580`)
  - Do: translucent rounded Panel (Transparency 40) with a self-clipping child
        covering all corners over (i) a form colour, (ii) a form image
        `TextureId::Managed(1)`, (iii) a PictureBox, (iv) a Label with
        `BackgroundColor`. Assert no notch mesh and no restore stroke in any
        corner, and that the notch pixel equals the same point rendered without
        the Panel.
  - Verify: `cargo test -p cobolt-forms --features render shape_dump` green.

- [x] **T10 — Delete the offscreen GL path** (R12, R13; AC8)
  - Files: delete `crates/cobolt-ide/src/panels/rounded_clip.rs`;
        `crates/cobolt-ide/src/panels/mod.rs:37`; `designer.rs:7177-7191`
        (hook), `:7247` (argument), `:7251-7257` (gate — loop always runs),
        `:7324-7325`; `crates/cobolt-ide/src/debug_settings.rs:47-48, :84,
        :127, :141, :232-240`; `crates/cobolt-ide/Cargo.toml:44-47`
        (`egui_glow`); `crates/cobolt-forms/src/render.rs:320-336`
        (`RoundedClipHook`), `:2543` (param), `:2623-2644`; callers
        `render.rs:12659`, `tests/gauge_surfaces.rs:66`,
        `tests/non_visual_controls_stay_in_the_designer.rs:57`,
        `tests/test_radio_designer_run_parity.rs:98`
  - Do: remove; add a `debug_settings` test that a `debug_settings.toml`
        carrying `rounded_clip = true` still loads (`#[serde(default)]` `:40`,
        pattern `:580-585`).
  - Verify: `cargo build --workspace --all-targets` clean; `grep -rn
        "rounded_clip\|egui_glow\|COBOLT_ROUNDED_CLIP\|RoundedClipHook" crates/`
        returns nothing; `cargo test -p cobolt-ide --bin cobolt-ide` and
        `cargo test -p cobolt-forms --features render` green; T2 goldens
        unchanged.

## Stage D — docs and release

- [x] **T11 — Docs & i18n** (spec §6)
  - Files: `.claude/skills/rounded-corners/SKILL.md` (line 3; item 6 `:39-43`;
        item 8 `:47-50`; rows `:64-65`; step 2 `:74-75`; invariants `:95-98`;
        new failure-mode row naming the AC1 test);
        `.claude/skills/rounded-corners/CORNER-BLEED-PLAYBOOK.md` (row `:132`,
        §5 `:267-283`, §8 `:353-354`, §7 code map);
        `crates/cobolt-forms/src/paint.rs:10661` (`parse_container_clip` doc:
        BORDER rect, not content rect — the producer is `render.rs:654-656`);
        `docs/developers-guide-en.md` **only if** it describes corner masking
        or the GL clip.
  - Do: replace every "GL capture/re-blit path" cure with the rule (a corner
        whose overlapping descendants self-clip is never repainted; name
        `corners_already_correct`); state the GL path was glow-only, never ran
        under wgpu, removed in 1.65.2. **i18n: none** — the removed switch's
        labels are plain `&'static str` (`debug_settings.rs:216-260`), no `Tr`
        keys added or removed.
  - Verify: `grep -rn "COBOLT_ROUNDED_CLIP\|ROUNDCLIP_\|GL capture" .claude/
        docs/` returns nothing outside CHANGELOG history; `grep -n "notch\|GL
        clip" docs/developers-guide-en.md` reviewed and either untouched or
        updated; `cargo test -p cobolt-ide --bin cobolt-ide` green (KB
        freshness test included).

- [x] **T12 — Finalize** (plan S8)
  - Files: `crates/cobolt-ide/src/version.rs` (`1.65.1 → 1.65.2`);
        `CHANGELOG.md`
  - Do: CHANGELOG entry in the house voice — the cause (children already draw
        to the arc; the mask repainted the form backdrop over them; in
        `inner-form2` that backdrop was not what was behind the panel), the
        rule, the dead GL path and why it never ran, the painters lifted, the
        measured allow-list. Full sweep: `cargo test -p cobolt-forms --features
        render`, `cargo test -p cobolt-ide --bin cobolt-ide`, `cargo test -p
        cobolt-form-host`, `cargo test -p cobolt-compiler` — **collect every
        `test result` line, never verdict from a failure grep**. Build the
        example: `rcrun build examples/PowerDemo3/PowerDemo3.project.toml`.
        Commit on `fixes` and push `main` + `fixes` per the approved plan.
  - Verify: every crate's `test result: ok`; `grep -rn "Unknown paint
        callback"` absent from a run log; **operator launch check** —
        `inner-form2` on designer, preview and run form: Panel-1's corners show
        the PictureBox behind them, no wedge (the agent does not drive the app).

## Done criteria

| AC | Covered by |
|---|---|
| AC1 | T3 (red) → T7 (green), corners recorded |
| AC2 | T5 — guardian tests unmodified |
| AC3 | T5 pure tests |
| AC4 | T2 goldens, unchanged through T5–T10 |
| AC5 | T4 baseline + T7 final tables in plan.md; equality assertion |
| AC6 | T3 three-surface test |
| AC7 | T6 |
| AC8 | T10 |
| AC9 | T8 plus the named guards, green at T12 |
| AC10 | T9 |

All ten checked, docs updated, one fix commit on `fixes`, pushed, and the
operator has seen the corners with their own eyes.
