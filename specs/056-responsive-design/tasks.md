# Tasks — Responsive design (spec 056, revision 2)

- **Status:** draft → awaiting approval (then `/implement`)
- **Plan:** ./plan.md (approved 2026-09-29)   **Date:** 2026-09-29
- **Line:** `1.80.x` — local; pushed only when the operator allows.

Ordered, small, independently-verifiable tasks. Each names the code it **reads
first** (R40/AC22 — re-read at the moment of the task; line numbers below are
from `be8657e` and will have moved), the files it touches, the requirements it
satisfies, and how to verify it. Check off as completed.

## Standing rules for every task

- **Gate G (operator's golden rule, §4.16):** from T0.4 onward, no task that
  touches `crates/cobolt-forms`, `crates/cobolt-form-host` or the designer
  canvas is done unless all of these are green and their `test result:` lines
  read (never a failure-grep):
  - `cargo test -p cobolt-forms --features render --test example_corpus_golden`
  - `cargo test -p cobolt-form-host corpus_golden`
  - `cargo test -p cobolt-codegen --test example_corpus_codegen`
  - the R80 list (T0.5).
- **Gate F (full sweep)** at the end of each phase:
  `cargo test -p cobolt-forms --features render --no-fail-fast`,
  `-p cobolt-form-host`, `-p cobolt-codegen`, `-p cobolt-runtime`,
  `-p cobolt-ide --bin cobolt-ide --no-fail-fast`, plus the R80 `cobolt-ide`
  integration tests. Expected reds are listed by name
  (`docs_embed::every_document_ships_in_every_language` until the 1.80
  translation cycle).
- **Every commit:** `z` bump in `crates/cobolt-ide/src/version.rs` + CHANGELOG
  entry; no fix mixed into this line; a property is never committed without
  its `Tr` ×6, `PROP_HELP` ×6 and KB entry, and a KB change never without
  `cargo run -p cobolt-ide --example build_chunked_kb` + `chunked.data`.
- **Every new test** prints a quantified summary of what it measured
  (GOLDEN RULE #7).
- A **fix** discovered on the way is not made here: it goes to `fixes` from
  `main`, and this line fast-forwards from `main` afterwards.

---

## Phase 0 — The golden, before any engine change (R78–R82)

Precondition: `git diff 0241901 HEAD -- crates` shows only version.rs.

- [ ] **T0.1 — Corpus golden harness (canvas / run / preview)** (R79, R82; AC2, AC36, AC39)
  - Read first: `crates/cobolt-forms/tests/inner_form2_corners_are_drawn_not_repaired.rs`,
    `tests/mask_corner_goldens.rs` (helpers `r2`, `fnv64`, `dump`),
    `cobolt-cli/src/form_gui.rs` theme resolution (~524-548),
    `render.rs` `RenderInput`/`RenderOutput`/`render_form*`/`render_faces`,
    `app.rs` preview `rail_view` call, `designer.rs` canvas `rail_view` call,
    `assets.rs` `set_base`, `fonts.rs` system-font loading.
  - Files: `crates/cobolt-forms/tests/example_corpus_golden.rs` (new),
    `crates/cobolt-forms/tests/goldens/056_corpus/**` (new).
  - Do: one `#[test]` over every `.cfrm` of `examples/PowerDemo3/forms` then
    `examples/PowerChat/forms` (sorted, `.bak` excluded); per form × size
    (0.75×/1×/1.5×, floor 64) × surface (canvas `render_faces` after
    `rail_view`; run `render_form` with a stringified live state; preview
    `render_form_with_chrome` after `rail_view`): fresh `Context`,
    `max_texture_side` 8192, `RawInput.time` pinned, two frames, record the
    second — sorted control id, rect (`r2`), font size (`ctrl_font_size` of
    the live control), shape digest (text shapes as size/family/colour/string
    hash only; Maps tile meshes excluded). Themes: `resolve_theme_id` +
    `discover_packs(<repo>/assets/themes)`. `COBOLT_WRITE_GOLDEN=1` rewrites.
    Prints: projects, forms, sizes, surfaces, rects and font sizes compared,
    differences, elapsed.
  - Verify: `COBOLT_WRITE_GOLDEN=1 cargo test -p cobolt-forms --features render --test example_corpus_golden`
    writes 62 files; a second run without the variable reports 0
    differences; run twice more — identical (determinism).

- [ ] **T0.2 — Host corpus golden (Window / Pane / occupant)** (R79; AC2, AC36)
  - Read first: `cobolt-form-host/src/host.rs` test module (`host_with_surface`,
    `raw`, `frame`, `fx_entrance_done`/`anim_started`/`lifecycle_sent`),
    `ensure_occupant`/`occupant_handle`, `last_control_rects`,
    `flatten_controls` (lib.rs), `Surface`.
  - Files: `crates/cobolt-form-host/src/host.rs` (test module only),
    `crates/cobolt-form-host/tests/goldens/056_corpus/**` (new).
  - Do: `#[test] fn corpus_golden` inside the test module: each of the 62 forms
    as root **Window**, root **Pane** (forms with a SideMenu), and as a
    ContentPane **occupant**, at the three sizes; record `last_control_rects`
    (+ font sizes); same write/compare switch and summary as T0.1.
  - Verify: `COBOLT_WRITE_GOLDEN=1 cargo test -p cobolt-form-host corpus_golden`
    then without it → 0 differences, twice.

- [ ] **T0.3 — Generated-COBOL snapshot** (R80; AC37)
  - Read first: `crates/cobolt-codegen/tests/generated_bytes_golden.rs`
    (`UPDATE_GOLDEN`), `cobolt-codegen/src/lib.rs` `generate`,
    `.gitignore:34` (`examples/*/generated/`).
  - Files: `crates/cobolt-codegen/tests/example_corpus_codegen.rs` (new),
    `crates/cobolt-codegen/tests/golden/corpus/**/*.cbl` (new, tracked).
  - Do: `generate(&load_form(p))` for the 62 forms vs the snapshot; first
    differing line reported; `UPDATE_GOLDEN=1` rewrites; summary printed.
  - Verify: write, then compare → 62/62 byte-identical.

- [ ] **T0.4 — Commit the golden** (AC36)
  - Files: the three tests + goldens; version.rs; CHANGELOG.
  - Verify: `git log` shows this commit **before** any commit touching
    `crates/*/src` (checked again in `/analyze`); `git diff 0241901 --stat -- crates/*/src`
    lists only version.rs.

- [ ] **T0.5 — R80 list in place** (R80; AC37)
  - Read first: every `crates/*/tests/*.rs` naming PowerDemo3/PowerChat;
    `powerchat_compiles.rs` (reads `generated/`, gitignored here).
  - Files: tests whose paths point at `~/Documents/PowerDemo3` or
    `forms/Inner-Forms/` (`maps_demo_compiles`, `charts_demo_compiles`,
    `rest_demo_compiles`, `snackbar_demo_compiles`, `test_maps_demo_form`) —
    repoint to `examples/PowerDemo3/forms/...` where the form exists there;
    otherwise leave and record the skip in the summary. PowerChat tests: run
    `cargo run -p cobolt-ide --example powerchat_regen` first in this worktree
    (revert its churn in `agents-form.cfrm`/`preview-form.cfrm`).
  - Verify: `cargo test -p cobolt-ide --test powerchat_compiles --test powerchat_runs --test props_demo_runs --test viewer_demo_compiles --test rest_demo_compiles --test maps_demo_compiles --test charts_demo_compiles --test snackbar_demo_compiles`,
    `cargo test -p cobolt-forms --features render --test inner_form2_corners_are_drawn_not_repaired --test two_surfaces_one_frame_share_no_widget_ids --test a_moved_container_carries_its_contents --test test_maps_demo_form`,
    `cargo test -p cobolt-runtime --test test_data_binding_runtime --test test_sql`
    — all green; skipped tests named. (Repointing a test is a test change the
    operator sees in the CHANGELOG, not a silent edit.)

## Phase 1 — Pure layout core (no egui, no callers)

- [ ] **T1.1 — Module skeleton, types, defaults table** (R22, R24, R85)
  - Read first: `cobolt-forms/src/lib.rs` module list and features;
    `model.rs` `Rect`, `Control`, `PropValue`, `content_rect`.
  - Files: `cobolt-forms/src/layout/{mod,defaults,props}.rs`, `lib.rs`.
  - Do: `LayoutInput`/`LayoutOutput` (rects, font_sizes, hidden, breakpoint,
    font_factor, min_size, placement, containers); `defaults.rs` with every
    default of plan §2; `props.rs` typed getters falling back to `defaults`.
  - Verify: `cargo build -p cobolt-forms` (no features) and
    `cargo test -p cobolt-forms layout::` green.

- [ ] **T1.2 — Anchoring** (R6–R10; AC4, AC5)
  - Files: `layout/anchor.rs`, tests in module.
  - Verify: 16 combinations × larger/smaller surface, expected rects derived in
    comments; idempotence `solve(solve(f,s),s)` and cold-vs-warm equality.

- [ ] **T1.3 — Docking** (R11–R15; AC6)
  - Files: `layout/dock.rs`.
  - Verify: Top/Left/Fill and Left/Top/Fill orders; anchored sibling against
    the full client rect.

- [ ] **T1.4 — Size limits and form minimum (pure)** (R16–R18; AC7, AC8 pure)
  - Files: `layout/{limits,minsize}.rs`.
  - Verify: MaxWidth keeps Left attachment; Fill with MinHeight; hand-derived
    minimum for a docked+anchored fixture; `MinFormWidth/Height` from
    `defaults` honoured.

- [ ] **T1.5 — Containers, recursion, padding (pure)** (R19–R21, R51; AC9 pure)
  - Read first: `model.rs` `content_rect` (~6235-6275), `tab_strip_extent`.
  - Verify: children of TabControl (4 `TabPosition`s), GroupBox, Panel inside
    `content_rect` of the laid-out container less padding; outputs form-space
    absolute.

- [ ] **T1.6 — Breakpoint selection, font factor, inverse mapping (pure)** (R58, R61, R66–R69, R33/R38 math)
  - Files: `layout/{breakpoints,fonts,inverse}.rs`.
  - Verify: selection exactly at thresholds; pin wins; Fluid clamps
    0.5×/1×/3× → 0.85/1.0/1.5; Stepped; `Min/MaxFontSize`, `ScaleFont=false`;
    inverse round-trip: `layout(inverse(layout(x)+d)) == layout(x)+d` for each
    placement kind; `me::Breakpoints` text round-trip.

- [ ] **T1.7 — No-static-values scan** (R85; AC42)
  - Files: `cobolt-forms/tests/layout_has_no_static_values.rs` (new).
  - Do: tokenise `src/layout/*.rs` except `defaults.rs`, skip comments,
    strings and `#[cfg(test)]` blocks; fail on numeric literals outside
    {0, 1, 2, 0.5}; also a test that changes a default and sees the layout move.
  - Verify: test green; deliberately adding a literal to `anchor.rs` (locally,
    not committed) turns it red.

- [ ] **T1.8 — Phase gate** — Gate G + Gate F; commit.

## Phase 2 — Model, persistence, migration, pane rows, KB/i18n

- [ ] **T2.1 — `Form.responsive` + form layout bag + XML** (R1, R2, R63, R87; AC1, AC29)
  - Read first: `model.rs` `Form`/`Form::new` (~7419-7595); `xml.rs`
    `OwnedEvent::FormStart`, `b"Form"` arm, `read_form`, `form_to_string`,
    `MenuPaneBackground` read/write and `menu_pane_background_round_trips_049`.
  - Files: `model.rs`, `xml.rs`.
  - Verify: AC1 (absent → false; `responsive="true"` → true; saved only when
    true); `<FormLayout>`/`<Breakpoints>` round-trip; defaults write nothing;
    Gate G (corpus unchanged).

- [ ] **T2.2 — `seed_layout_props` + Anchor→Locked migration** (R34–R36, R16, R11, R51; AC16, AC17)
  - Read first: `Control::new` universal props (~4712-4733); `seed_missing_props`
    (~643-889) key list; `seed_theme_owned_appearance` (one-path pattern);
    `parse_prop_value`; every `is_anchored` reader (grep); `snackbar_template.rs`;
    `every_control_seeds_every_theme_owned_property.rs`.
  - Files: `model.rs`, `xml.rs`, `designer.rs` (drag-lock readers →
    `is_locked()`), `properties.rs` (lock row → `Locked`), `editor.rs`
    autocomplete fixtures, `tests/snackbar_template.rs`; delete `is_anchored`.
  - Verify: AC16 via XML load (`true`/`false`/`Top,Left`); AC17; drift test
    over `ControlType::ALL` covers the new keys; Viewer keeps its own `Layout`
    and gets no `LayoutMode`; Gate G.

- [ ] **T2.3 — Save omits layout defaults** (R87; AC44)
  - Files: `xml.rs` writer.
  - Verify: AC44; every corpus form round-trips (load → save → load equal);
    Gate G.

- [ ] **T2.4 — Properties-pane Layout section (controls)** (R32, R83; AC40 controls)
  - Read first: `properties.rs` section order (~3860-4021), row helpers
    (`bool_prop_row`, `int_prop_row`, `combo_prop_row`, `section_header`),
    conditional-row idioms.
  - Files: `properties.rs`, `i18n.rs` (labels, values, the "off" hint).
  - Do: section only when `form.responsive` (hint otherwise); Anchor
    checkboxes + Dock when parent `Absolute`; the parent layout's item props
    otherwise; limits; font props; `Padding*`; container props on containers;
    owner-positioned controls hidden (R27).
  - Verify: AC40 designer test (row per property, edit changes model, marks
    dirty); `cargo test -p cobolt-ide --bin cobolt-ide properties`.

- [ ] **T2.5 — Form-level properties + breakpoint editor** (R1, R4, R83; AC40 form)
  - Read first: `designer.rs` `set_form_prop`/`set_form_prop_direct`/
    `get_form_prop`/`FORM_PROP_KEYS`/`canonical_form_prop_key`;
    `agent.rs::form_property_valid`; test `form_property_lists_agree`;
    `properties.rs` form rows (~10032-10770).
  - Files: `designer.rs`, `properties.rs`, `agent.rs`, `i18n.rs`.
  - Verify: toggling `Responsive design` marks dirty and moves no rect (R4);
    breakpoint editor add/rename/remove; `form_property_lists_agree` green.

- [ ] **T2.6 — Hover help + KB entries for every new property** (AC19 partial, AC20 partial)
  - Read first: `prop_help_data.rs`, `prop_help.rs` tests (~74, ~101);
    compiler `UNIVERSAL_PROPS`, `property_reference_for`, text at ~4581-4600,
    Anchor doc (~5160), coverage test (~10560).
  - Files: `prop_help_data.rs`, `cobolt-compiler/src/lib.rs`,
    `assets/knowledge/chunked.data`.
  - Do: help ×6 for every new key; KB entries read/write; rewrite "no anchoring
    or docking" and the Anchor/Locked entries; rebuild the chunked store.
  - Verify: `every_control_property_is_explained_in_six_languages`,
    `every_form_property_is_explained_in_six_languages`,
    `every_control_property_is_documented`,
    `prebuilt_chunked_kb_matches_the_published_documentation` green.

- [ ] **T2.7 — Phase gate** — Gate G + Gate F; commit(s).

## Phase 3 — One font resolver (R70)

- [ ] **T3.1 — Resolver + every paint site** (R70, R71; AC33)
  - Read first: `paint.rs` `ctrl_font_size` and every caller; `render.rs`
    sites (~7461, 8925, 9090, 11671, 11687); chart (~12806, 12992,
    `CHART_FONT_SCALE`); `model.rs` tab strip (~6282); Viewer (~10791);
    `snackbar.rs` (~1076); `PropValue::as_i64`. Grep again for
    `"FontSize"` reads — the list above is the plan's, not proof.
  - Files: `layout/fonts.rs`, `paint.rs`, `render.rs`, `model.rs`,
    `snackbar.rs`.
  - Do: `resolve(ctrl, FontSite)` = `_EffectiveFontSize` if present, else each
    site's own default and clamp exactly as today; trims and accepts
    decimals. Viewer export (`paint_viewer_export`) excluded, documented.
  - Verify: AC33 — source scan (no `FontSize` read outside `layout/fonts.rs`
    and tests) + a render walk of every `ControlType` asserting galley font
    sizes; Gate G unchanged (corpus has no decimal sizes).

- [ ] **T3.2 — Phase gate** — Gate F; commit (CHANGELOG names the decimal
  behaviour change: `"18.5"` no longer paints at 4 pt).

## Phase 4 — Surfaces lay out responsive forms (R23, R25–R28, R43, R3, R5, R18)

- [ ] **T4.1 — `apply.rs`: prepare, laid-out list, `LaidOutState`, AutoSize gate** (R22, R26, R28, R62)
  - Read first: `render.rs` `FormState`, `merge_props`, `live_control`,
    `resolved_rect`, `moved_ancestor_offset`, `autosize_rect` call (~2312);
    `paint.rs` `autosize_rect`.
  - Files: `layout/apply.rs` (render-gated), `render.rs` (gate only).
  - Verify: unit tests for the moved-ancestor rule in `prepare`; AutoSize at
    effective size; Gate G.

- [ ] **T4.2 — Host root Window + Pane** (R23, R25, R43)
  - Read first: `host.rs` construction shift (~459-490), `ui_impl` render
    (~5279-5441), size-observation block (~4918-5004), `stretch_window_bars`
    call, `painted_controls`/`rail_view`.
  - Files: `host.rs`, `sidebar.rs` (`pane_shift` factored out, shared).
  - Verify: responsive fixtures in the host test module; Gate G (0b
    unchanged).

- [ ] **T4.3 — `child_frame`: occupants and child windows; fx face** (R23, R26)
  - Read first: `child_frame` (~2592-2738), `paint_fx_frame`/`paint_face`
    (~4170-4243).
  - Files: `host.rs`.
  - Verify: host tests; Gate G.

- [ ] **T4.4 — Preview** (R23)
  - Read first: `app.rs` `show_preview_window` (~15986-16355), `PreviewState`.
  - Files: `app.rs`.
  - Verify: preview surface in the corpus harness (responsive copy); Gate G.

- [ ] **T4.5 — Window minimum inner size** (R18; AC8 window half)
  - Read first: viewport builders `host.rs` (~300, ~4081), `shell.rs` (~1409).
  - Files: `host.rs`, `shell.rs`.
  - Verify: builder receives `max(min_size, MinForm*)`; `MinInnerSize` sent
    when it changes; no window resizes itself (GOLDEN RULE).

- [ ] **T4.6 — Responsive-on corpus check** (R81, R3, R5; AC3, AC38)
  - Files: `example_corpus_golden.rs`, host `corpus_golden`.
  - Do: for each of the 62 forms, a copy with only `responsive="true"`
    rendered at its designed size on canvas/run/preview (0a) and
    Window/Pane/occupant (0b) must equal the golden.
  - Verify: 0 differences, summary printed.

- [ ] **T4.7 — Surface parity + precedence fixtures** (R23, R26, R27, R43; AC10 non-designer, AC11, AC12, AC23)
  - Verify: one responsive fixture (anchors, dock, AutoSize under Fluid,
    Splitter, collapsed SideMenu, repeating group, COBOL-moved container)
    yields identical `control_rects` and font sizes on Window/Pane/occupant/
    preview; owner-positioned controls ignore their layout props; MenuBar/
    StatusBar laid out, `stretch_window_bars` not called (and still called for
    non-responsive); responsive copy of `a_moved_container_carries_its_contents`.

- [ ] **T4.8 — Phase gate** — Gate G + Gate F; commit.

## Phase 5 — Flex, Flow, Grid (R49–R57)

- [ ] **T5.1 — Flex solver** (R52–R54; AC24)
  - Read first (reference only, not reused): `viewer.rs` flex types,
    `paint.rs` `paint_box_content` row branch.
  - Files: `layout/flex.rs`.
  - Verify: AC24 cases, hand-derived expectations in comments.

- [ ] **T5.2 — Flow on the flex solver** (R57; AC26)
  - Files: `layout/flex.rs`.
  - Verify: AC26.

- [ ] **T5.3 — Track parser + grid solver** (R55, R56; AC25)
  - Files: `layout/{tracks,grid}.rs`.
  - Verify: AC25.

- [ ] **T5.4 — Wire into the tree walk; ContainerGeom; minimum** (R49, R50, R53 content sizing; AC8/AC9 flex-grid halves)
  - Files: `layout/mod.rs`, `minsize.rs`.
  - Verify: nested flex in grid in docked panel fixture; `ContainerGeom`
    filled; Gate G.

- [ ] **T5.5 — Viewer untouched** (R49; AC27)
  - Verify: `cargo test -p cobolt-forms --features render --test a_pages_flex_and_grid_are_laid_out` green, unchanged file.

- [ ] **T5.6 — Phase gate** — Gate G + Gate F; commit.

## Phase 6 — Breakpoints, type scaling, system text factor (R58–R72)

- [ ] **T6.1 — Overrides applied before layout; hidden = absent** (R59, R60, R62, R64 override half; AC28)
  - Files: `layout/breakpoints.rs`, `apply.rs`.
  - Verify: AC28 at 480/800/1280 px (sidebar Left→Top, grid 2→1 columns,
    panel hidden takes no slot).

- [ ] **T6.2 — Fluid/Stepped end to end** (R66–R69, R72; AC32)
  - Verify: AC32 on host surfaces; no designed rect changes.

- [ ] **T6.3 — System text factor provider** (R68; AC34)
  - Read first: workspace `Cargo.toml` for an existing `windows-sys`/registry
    dependency; `host.rs` `native_pixels_per_point` read.
  - Files: `cobolt-form-host/src/text_scale.rs` (new), `Cargo.toml`
    (target-gated `windows-sys` Registry feature), designer uses the same
    provider.
  - Verify: AC34 with an injected 1.25; `cargo build -p cobolt-form-host`;
    macOS returns 1.0 (documented); CHANGELOG notes the dependency.

- [ ] **T6.4 — Phase gate** — Gate G + Gate F; commit.

## Phase 7 — COBOL at run time (R37–R39, R46–R48, R64, R84)

- [ ] **T7.1 — `CtrlState.written`** (R64)
  - Read first: `state.rs` `CtrlState`, `from_control`, `set`; `FormState`.
  - Verify: unit tests; COBOL `Visible` write beats an override (AC28 part).

- [ ] **T7.2 — Geometry writes through the inverse mapping** (R38; AC43)
  - Read first: `host.rs` `apply_interpreter_update` (~2097-2218).
  - Verify: AC43 (`ADD 10 TO Btn::X` twice on a Right-anchored button → +20 px,
    reads back as written).

- [ ] **T7.3 — Mirror laid-out rects; event order** (R37, R39, R47; AC18, AC31)
  - Read first: `mirror_size` and the size block; `interpreter.rs`
    `drain_input` (~2811); `FORM_EVENT_GROUPS`; compiler event text + guard;
    codegen generic form-event WHEN (~2611-2645).
  - Files: `host.rs`, `model.rs` (`onBreakpointChanged`, count test 57→58),
    compiler `lib.rs` (event docs), `prop_help_data.rs`, `chunked.data`.
  - Verify: AC18, AC31 in the host test module; only changed rects mirrored
    (count asserted); no mirror at the designed size.

- [ ] **T7.4 — Form properties writable; pins** (R46, R84; AC41)
  - Read first: `apply_form_window_update` (~2049); `seeding.rs` form entries
    (~241-275).
  - Files: `host.rs`, `seeding.rs`.
  - Verify: AC41 — every new property written from COBOL and read back;
    layout reflects it the same frame; Breakpoint pin/unpin (event on a pin
    that changes it), FontScale fix/unfix, `Breakpoints` round-trip.

- [ ] **T7.5 — A PowerChat-style run test** (R48)
  - Files: `cobolt-ide/tests/responsive_runs.rs` (new; a temp copy of a
    PowerChat form made responsive, never the committed example).
  - Verify: a hand-written `onResize` still composes with the layout.

- [ ] **T7.6 — Interpreter/binary parity** (skill `interpreter-binary-parity`)
  - Verify: rcrun run-form, embedded child forms and the compiled binary all go
    through the changed host paths (read `form_gui.rs`, compiler `run_form_app`);
    a build-and-run smoke test of a responsive fixture (AC10 binary half).

- [ ] **T7.7 — Phase gate** — Gate G + Gate F; commit.

## Phase 8 — Designer (R29–R31, R33, R44, R45, R65, R88)

- [ ] **T8.1 — `canvas_view_controls`, view size, grip = view size** (R29, R88; AC45)
  - Read first: `designer.rs` canvas size (~6980), `rail_view_controls`
    (~11816), `render_faces` call (~8382) and its `control_rects` consumers,
    grip (~12986-13004), `hit_top_id`, `tab_strip_hit`, secondary selection.
  - Verify: AC45; live reflow on grip drag; non-responsive canvas unchanged
    (Gate G canvas surface).

- [ ] **T8.2 — View at bar (presets + breakpoints)** (R30; AC14)
  - Verify: AC14.

- [ ] **T8.3 — Inverse drag/resize** (R33; AC13)
  - Read first: `handle_drag` move (~12827-12912) and resize (~12790-12937),
    `apply_resize`, `Cmd::MoveMany`/`ResizeControl`, nudge/align/paste writers.
  - Verify: AC13; undo restores designed values.

- [ ] **T8.4 — Anchor gizmo** (R31; AC15)
  - Verify: AC15 (hit-test unit test + shape dump); *(manual)* look.

- [ ] **T8.5 — Flex/grid overlays and drag-to-reorder** (R44, R45)
  - Verify: dropping between items writes `Order`; on a grid writes
    `GridColumn/GridRow`; never X/Y.

- [ ] **T8.6 — Override editing** (R65; AC30)
  - Verify: AC30; undoable `SetOverride`.

- [ ] **T8.7 — Designer surface parity** (AC10 designer half)
  - Verify: canvas `control_rects` equal the host's for the T4.7 fixture at the
    same view size.

- [ ] **T8.8 — Phase gate** — Gate G + Gate F; `bench_render_frame` before/after
  reported; commit.

## Phase 9 — Responsive by default for new projects (R73–R77)

- [ ] **T9.1 — `[forms] responsive` + default breakpoints** (R73, R74)
  - Read first: `project_model.rs` `FormsConfig`, `new_project_defaults`,
    `Default`; compiler copy (~789-820) and its baking into `main.rs`.
  - Files: `project_model.rs`, `cobolt-compiler/src/lib.rs`.

- [ ] **T9.2 — New forms written responsive** (R75)
  - Read first: `app.rs` `create_new_form`/`save_new_form_to` (~13424-13510),
    paste-form path.

- [ ] **T9.3 — Settings checkbox** (R76)
  - Read first: `settings_form.rs` focus-ring row pattern.
  - Files: `settings_form.rs`, `i18n.rs`.

- [ ] **T9.4 — Verify** (R77; AC35)
  - Verify: AC35 through the IDE's own creation path; no upgrade registered
    (`project_upgrade.rs` UPGRADES unchanged); Gate F; commit.

## Phase 10 — Documentation, KB, i18n completeness

- [ ] **T10.1 — Developer's Guide chapter "Responsive design"** (AC21)
  - Files: `docs/developers-guide-en.md` (anchoring & docking, LayoutMode
    flex/grid/flow, breakpoints & overrides, font scaling, COBOL properties and
    events, the new-project default, migrating a hand-written `onResize`),
    mermaid where it fits, `📷 Screenshot needed` placeholders; support-matrix
    rows under `PRC`. Translations are **not** patched — regenerated at the
    1.80 release (GOLDEN RULE #8; the red `every_document_ships_in_every_language`
    is the expected signal).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` (only the
    expected red).

- [ ] **T10.2 — KB form-property table + final rebuild** (AC20)
  - Files: `cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data`.
  - Verify: KB coverage tests + freshness test green.

- [ ] **T10.3 — i18n completeness** (AC19)
  - Files: `i18n.rs` (`sample()` list extended with every new field).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide i18n` green.

## Finalize

- [ ] **T11 — Full sweep, manual check, analyze**
  - Do: Gate G + Gate F on the final tree; `/analyze` (AC22: plan citations and
    task "Read first" lists); `/docsync`.
  - Manual (operator, plan §6): PowerChat CHAT-FORM made responsive with
    anchors replacing its `onResize`; resize, collapse the rail, cross 600/1024;
    grip, View at, one override; PowerDemo3 forms run unchanged.
  - Verify: every AC1–AC45 checked below; push `1.80.x` only when the operator
    allows.

## Acceptance-criteria coverage

| AC | Task(s) | AC | Task(s) | AC | Task(s) |
|---|---|---|---|---|---|
| AC1 | T2.1 | AC16 | T2.2 | AC31 | T7.3 |
| AC2 | T0.1, T0.2 | AC17 | T2.2 | AC32 | T6.2 |
| AC3 | T4.6 | AC18 | T7.3 | AC33 | T3.1 |
| AC4 | T1.2 | AC19 | T2.4, T2.6, T10.3 | AC34 | T6.3 |
| AC5 | T1.2 | AC20 | T2.6, T7.3, T10.2 | AC35 | T9.4 |
| AC6 | T1.3 | AC21 | T10.1 | AC36 | T0.1–T0.4 |
| AC7 | T1.4 | AC22 | T11 (`/analyze`) | AC37 | T0.3, T0.5 |
| AC8 | T1.4, T4.5, T5.4 | AC23 | T4.7 | AC38 | T4.6 |
| AC9 | T1.5, T5.4 | AC24 | T5.1 | AC39 | T0.1, T0.2 |
| AC10 | T4.7, T7.6, T8.7 | AC25 | T5.3 | AC40 | T2.4, T2.5 |
| AC11 | T4.7 | AC26 | T5.2 | AC41 | T7.4 |
| AC12 | T4.7 | AC27 | T5.5 | AC42 | T1.7 |
| AC13 | T8.3 | AC28 | T6.1, T7.1 | AC43 | T7.2 |
| AC14 | T8.2 | AC29 | T2.1 | AC44 | T2.3 |
| AC15 | T8.4 | AC30 | T8.6 | AC45 | T8.1 |

## Done criteria

All acceptance criteria in spec.md are checked; Gate G has been green on every
engine commit; Gate F is green (expected reds named); docs, KB and i18n updated;
every commit on `1.80.x` carries its `z` bump and CHANGELOG entry; the branch is
pushed only when the operator allows.
