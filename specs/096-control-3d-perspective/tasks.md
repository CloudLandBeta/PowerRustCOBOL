# Tasks — 3D perspective for display controls

- **Status:** implemented in 1.91.39 (T16 dropped; T18's sweep recorded in the CHANGELOG)
- **Plan:** ./plan.md   **Date:** 2026-10-13

Ordered, small, independently-verifiable tasks. Each names the files it touches, the
requirement(s) it satisfies, and how to verify it. The project stays green after each one:
until T7 and T8 hook the renderers, nothing a user can see changes. **T1 is a go/no-go gate:
if it fails, stop and take plan §4 alternative (b) before anything else is written.**

- [x] **T1 — Spike: can the captured shapes be tessellated and projected?** (R7, plan §5 risk 1)
  - Files: `crates/cobolt-forms/tests/perspective_spike.rs` (kept as a regression test).
  - Do: draw a BarChart, a Label and a PictureBox through the real painter into a headless
    `egui::Context`; read the shapes added to the layer (`graphics_mut` → `PaintList::next_idx`
    / `all_entries`); tessellate each with `epaint::Tessellator::new(ppp, options, font_tex_size,
    prepared_discs)`; compare the glyph quads and rect fills with the primitives egui's own
    `ctx.tessellate` produces for the same shapes (same vertex count, same positions within
    0.01 pt, same UVs); then add the meshes back and check the frame renders the same.
  - Verify: `cargo test -p cobolt-forms --features render --test perspective_spike -- --nocapture`
    prints, per control, shapes captured / meshes made / glyph quads compared / worst position
    difference. **Go** = every difference ≤ 0.01 pt and text present. **No-go** = stop; report.

- [x] **T2 — The properties exist, and mean the same everywhere** (R1, R2, R12 write side, R13, R16, R18; AC1, AC13)
  - Files: `crates/cobolt-forms/src/model.rs`.
  - Do: `ControlType::has_perspective()` (the 12 of spec R1, as one `matches!`); consts for the
    three names, their ranges and defaults; seed `RotateX`/`RotateY` (`Int(0)`) and
    `PerspectiveDistance` (`Int(50)`) in `Control::new` for those types only; in
    `property_runtime_text`, parse the three, clamp to range, return the whole number as text
    (a non-numeric text is passed through untouched, as a boolean's is).
  - Verify: `cargo test -p cobolt-forms --features render` and `cargo test --manifest-path
    tests/controls/Cargo.toml` green (the generic `assert_control` round-trips the new seeds);
    a new test asserts: exactly the 12 types report `has_perspective` (count asserted), the
    other types do not seed or list the properties, a Button loaded from a `.cfrm` that names
    `RotateY` is unchanged and unaffected, `RotateX` `"200"` normalises to `"60"` and `"-99"`
    to `"-60"`, and a theme switch (`THEME_OWNED_PROPS` appliers) leaves the three as they were;
    the layout rectangle, tab order and event bindings are equal before and after a write.

- [x] **T3 — The projection, as pure geometry** (R3, R4, R5; AC3, AC4)
  - Files: `crates/cobolt-forms/src/perspective.rs` (new), `crates/cobolt-forms/src/lib.rs`.
  - Do: `Perspective { rx, ry, distance }` with `of(&Control) -> Option<Perspective>` (`None`
    for the other types, the neutral triple and non-finite text; clamps), `project(centre, size,
    p) -> Pos2` as plan §1 (`K = 2.0` as a named const), `quad(rect) -> [Pos2; 4]`,
    `contains(quad, p)` (convex quad test), edge-length helpers.
  - Verify: `cargo test -p cobolt-forms --features render --test perspective_projection --
    --nocapture` prints: the four edge lengths for `RotateY` ±30 and `RotateX` ±30; the size and
    edge ratio at distance 0/25/50/75/100; the centre exactly fixed; the smallest `D + z₂` and the
    convexity over a 61 × 61 × 101 grid of the three properties and several aspect ratios.

- [x] **T4 — Projecting a mesh** (R7; AC5 in part)
  - Files: `crates/cobolt-forms/src/perspective.rs`.
  - Do: `warp_mesh(&Perspective, centre, Mesh) -> Mesh`: positions projected; a mesh whose
    vertices share one colour and one UV is moved as it is, any other has its triangles cut so
    no edge exceeds 24 points (UVs and colours interpolated in the unprojected space).
  - Verify: unit tests in `perspective.rs` print: a flat quad keeps its four outline corners; a
    textured 200 × 100 quad at `RotateY` −30 — the point halfway along its top edge in the image
    lands at the perspective midpoint of the projected edge (not at the screen midpoint), the
    triangle count before/after, no NaN, no zero-area triangle.

- [x] **T5 — Clipping, in control space and in screen space** (R9; AC7 in part)
  - Files: `crates/cobolt-forms/src/perspective.rs`.
  - Do: `clip_triangles_to_rect` (Sutherland–Hodgman against a rectangle); `clip_case(shape_clip,
    outer) -> {PreClip(rect), Keep(rect)}` as plan §1: tighter than `outer` → pre-clip and carry
    `outer`; `outer` or wider → no pre-clip, keep that clip.
  - Verify: a table test prints each case (equal, tighter, wider, partly overlapping, empty) with
    the chosen rule; a triangle clipped to a rectangle gives the right polygon area (checked
    against a brute-force sample of 10 000 points).

- [x] **T6 — `PerspectiveScope`: capture, tessellate, project, put back** (R6, R7, R9; AC2, AC5, AC7)
  - Files: `crates/cobolt-forms/src/perspective.rs`.
  - Do: `PerspectiveScope::enter(painter, rect, Option<Perspective>)` — a no-op value when the
    perspective is `None` (R6: nothing is read, nothing is allocated); with one, it records the
    layer and `next_idx`; `Drop` takes the shapes added since, builds a `Tessellator` (the context's
    `pixels_per_point`, `tessellation_options`, `font_image_size`, `prepared_discs`), turns each
    into meshes (recursing into `Shape::Vec`, skipping `Noop`, leaving a `Callback` as it is), applies
    T5 and T4, writes the meshes back with `mutate_shape` (one shape per index), and records the
    quad. Never nests `graphics_mut` closures.
  - Verify: `cargo test -p cobolt-forms --features render --test perspective_shapes -- --nocapture`:
    at the neutral triple the shapes in the layer are **identical** to a run without the scope (the
    twelve types, one line each); a tilted BarChart, PictureBox, Label, Shape and Line: every
    straight edge within 0.5 px of the quad's edge, no degenerate glyph, the image's four corners on
    the outline's four corners; none of the twelve types paints a `Shape::Callback`.

- [x] **T7 — Hook the designer/static painter** (R8, R11; AC2, AC6, AC9)
  - Files: `crates/cobolt-forms/src/render.rs` (`render_faces_with`; `RenderOutput.hit_quads`).
  - Do: wrap the `draw_control` call in a scope built from `Perspective::of(&face)` and `screen`;
    leave `control_rects` the designed rectangle.
  - Verify: the 75-form corpus goldens (`cobolt-forms`) pass **unregenerated**; the parity guard
    `engine_reference_form_parity_static_vs_faces` is extended with a tilted chart, picture and
    shape and prints the compared vertex count; a designer test (`cobolt-ide`, `designer.rs`) shows
    a tilted control's selection rectangle and handles on the designed rectangle and a handle drag
    resizing it.

- [x] **T8 — Hook the run painter** (R8, R9, R15; AC6, AC7, AC11 in part)
  - Files: `crates/cobolt-forms/src/render.rs` (`render_form`).
  - Do: the same scope around each control's painting in `render_form`'s loop, built after the
    `screen`/`clip` and the animation transform are known, so it also covers the shadow ring and the
    arms that paint through other painters of the same layer; record `hit_quads`.
  - Verify: the 75-form corpus goldens of `cobolt-form-host` pass unregenerated; a tilted chart
    inside a Panel paints nothing outside the Panel's content rectangle and one on a layer nothing
    outside the form's (painted bounds printed); a later control in z-order paints over it; an
    animated Animator advances between two frames of the test clock while tilted; a control with an
    animation transform (shift + scale) and a tilt draws the tilt about the moved centre.

- [x] **T9 — A tilted Label** (R14; AC11)
  - Files: `crates/cobolt-forms/src/render.rs` (the Label arm).
  - Do: first **read** `collect_tab_targets` and the arm and write down what keeps `onGotFocus` and the
    tab stop alive when the caption is not hosted; then, while a Label is tilted, call `draw_control`
    (caption painted, inside the scope) in place of `draw_control_capturing_label` and skip the hosting.
  - Verify: a test prints, for a tilted and an untilted Label: caption shapes present / selectable
    text hosted / tab target present / `onGotFocus` announced — equal except "selectable".

- [x] **T10 — Pointer events where the control is** (R10; AC8)
  - Files: `crates/cobolt-forms/src/render.rs`.
  - Do: `hit(out, id, rect, p)`; use it in `control_pointer_events` (`over`), `owns_pointer_at`,
    `controls_under_the_owner`, `paints_at`; the `ui.interact` rectangle of the arms of the twelve types
    becomes the quad's bounding box while tilted.
  - Verify: `perspective_hit.rs` prints, for a tilted Shape, the table of pointer positions (inside both,
    inside the quad only, inside the rectangle only, outside both) and the events each gives; the
    spec 091 layer/owner tests still pass; a tilted chart over a Button hands a click inside the quad
    to the chart and a click on the Button outside the quad to the Button.

- [x] **T11 — Writing the properties from COBOL, in all three hosts** (R12; AC10)
  - Files: `crates/cobolt-form-host/tests/…` (new test), no production change expected.
  - Do: **read** the skill `interpreter-binary-parity` first; run a form whose handler does `MOVE 30 TO
    CHART-1::RotateY`, `MOVE 200 TO CHART-1::RotateX` and reads both back, through the form host's
    run path and the compiled-binary path (`run_form_app`).
  - Verify: the next frame's outline differs as T3 predicts; the read-back is 30 and 60; both hosts
    print the same numbers.

- [x] **T12 — The Properties pane** (R17; AC12 in part)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `crates/cobolt-ide/src/prop_help_data.rs`,
    `crates/cobolt-ide/src/ui_text_data.rs`.
  - Do: a "3D perspective" section after the type-specific one for the twelve types, with
    `int_prop_row` for `RotateX` and `RotateY` (−60..=60, default 0) and `PerspectiveDistance`
    (0..=100, default 50); three `PropHelp { ty: "", … }` entries (the sign convention of R3 in the
    help text), six languages; the section title in `ui_text_data.rs`, six languages.
  - Verify: `cargo test -p cobolt-ide` — the property-help audit (`every_control_property_is_explained_in_six_languages`) is green; a drawing test shows the section
    for each of the twelve types and for none of the others, in six languages, with no word of it left
    in English; `ui_text_tests` green.

- [x] **T13 — System KB** (R17; AC12 in part)
  - Files: `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`.
  - Do: property arms for `RotateX` | `RotateY` and `PerspectiveDistance` (domain, one-sentence meaning,
    the sign table); then `cargo run -p cobolt-ide --example build_chunked_kb`.
  - Verify: `cargo test -p cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green;
    `cargo test -p cobolt-compiler` green; the record count before/after printed.

- [x] **T14 — What it costs** (R19; AC14)
  - Files: `crates/cobolt-forms/tests/bench_render_frame.rs` (extend).
  - Do: a scene of 1 and of 8 BarCharts, untilted and tilted (RotateY −30, RotateX 8, distance 60).
  - Verify: `cargo test -p cobolt-forms --features render --release --test bench_render_frame --
    --nocapture` prints the frame time of the four scenes; the numbers go into the CHANGELOG as
    measured; a scene with **no** tilted control is within noise of the previous baseline.

- [x] **T15 — The reference scene, looked at** (R7, R17; AC12)
  - Files: `assets/images/screenshots/` (the picture, where the Guide keeps its images), a scratch `.cfrm` outside the repo.
  - Do: build the *Revenue by quarter* scene (a BarChart, two series, a dark backdrop), render it
    with `render_form` at `RotateY` −30, `RotateX` 8, distance 60; **look at it** beside the operator's
    picture; tune `K` (and the 24-point subdivision step if the arc shows facets); render again.
  - Verify: the picture is in the repo; the author states in the CHANGELOG what was seen (edge ratio
    read off the picture, text legible, border following the tilt, the arc of the rounded corners smooth).

- [~] **T16 — (optional) The demo form** (R17) — **dropped, as the task allows** (a smaller change: no PowerDemo3 project entry and no two new goldens; the Guide's COBOL example and the reference picture carry the explanation)
  - Files: `examples/PowerDemo3/forms/General/perspective-form.cfrm`, `examples/PowerDemo3/PowerDemo3.project.toml`,
    the two goldens `General__perspective-form.cfrm.txt` (`cobolt-forms`, `cobolt-form-host`), the corpus
    count in `every_example_form_round_trips_056` (75 → 76).
  - Do: a BarChart with three Sliders that write `RotateX`, `RotateY` and `PerspectiveDistance`, in the
    house COBOL style (property access with `::`, a comment before every `CALL`).
  - Verify: the corpus tests green with 76 forms; the demo's four sliders move the chart in `run_form`.
    **Drop this task for a smaller change.**

- [x] **T17 — Docs** (R17)
  - Files: `docs/developers-guide-en.md`.
  - Do: a subsection (*Painting and layout ▸ 3D perspective*): what the three properties do, the sign
    table, the 12 types and why not the others, the reference picture, a COBOL example with the
    extension syntax, ⚠️ caveats (extremes enlarge the near edge; selection stays on the flat
    rectangle; a tilted Label's text is not selectable). English only; delete the translations the
    edit invalidates (GOLDEN RULE #8) and say so.
  - Verify: the Guide builds in the IDE's viewer (the `docs_embed` tests that are green stay green);
    every claim in it has a test above.

- [ ] **T18 — Finalize** (all)
  - Files: `crates/cobolt-ide/src/version.rs` (**1.91.39** — 1.91.36–38 went to `fixes` first), `CHANGELOG.md`.
  - Do: bump; the entry states what shipped, the measured numbers (T14, T3, T6, T8, T10), what the author
    saw (T15), and what was **not** checked on screen by the operator.
  - Verify: `cargo test -p cobolt-forms --features render`, `-p cobolt-form-host`, `-p cobolt-compiler`,
    `-p cobolt-ide`, `cargo test --manifest-path tests/controls/Cargo.toml`, and `cargo check --workspace
    --all-targets` — every `test result:` line read, the one expected red (`docs_embed`) named; the
    manual check of plan §6 (Designer, Preview, Run Form, a built binary) listed for the operator.

## Done criteria
All acceptance criteria in spec.md are checked, tests pass, docs updated, and the change is split into
fix/feature commit(s) per the operator's rules (do **not** commit/push unless the operator asks).
Coverage of the acceptance criteria: AC1 T2 · AC2 T6/T7/T8 · AC3 T3 · AC4 T3 · AC5 T4/T6 · AC6 T7/T8 ·
AC7 T5/T8 · AC8 T10 · AC9 T7 · AC10 T11 · AC11 T8/T9 · AC12 T12/T13/T15 · AC13 T2 · AC14 T14.
