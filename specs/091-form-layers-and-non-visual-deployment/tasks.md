# Tasks — Form layers and the Non-Visuals tab (spec 091)

- **Spec:** ./spec.md  **Plan:** ./plan.md  **Date:** 2026-10-09
- **Status:** in progress — T1–T22 done (slices 4 and 5 committed); T23 onward to do
- **Branch:** `features-form-layers` (worktree `.claude/worktrees/form-layers`). Feature ⇒ `features` line, one `z` bump and one CHANGELOG entry per commit. Merge to `main` and push only when asked, never 09:00–18:00 São Paulo Mon–Fri.
- Every verify line assumes `CARGO_TARGET_DIR=/Users/emersonlopes/Documents/PowerRustCOBOL/target CARGO_INCREMENTAL=0`, one crate at a time, test executables in `target/debug/deps` deleted between crates.

## Slices 0–3 — model, engine, pointer, run time (done)

- [x] **T1 — Baseline for AC1** (R2, R39; AC1) — `cfc5f82`, 1.90.27
  - Files: `crates/cobolt-forms/tests/every_example_form_saves_unchanged_091.rs`; spec approved.
  - Verify: `cargo test -p cobolt-forms --features render --test every_example_form_saves_unchanged_091` — 97 forms, 18 equal their file; the count must not drop.

- [x] **T2 — The layer model and its file format** (R1–R8, R39, R40; AC1–AC4, AC13) — `cd5eca8`, 1.90.28
  - Files: `cobolt-forms/src/{model.rs,xml.rs,containers.rs,nv_grid.rs,lib.rs}`; `tests/layers_model_091.rs`.
  - `Layer`, `Form.layers`, `Control.layer`, names/reserved/64 cap, add/rename/re-stack/move, R8 container rule, `<Layer/>` and `layer=` written only when used (never `Visible`), `render_order_in`, pure `nv_grid`.
  - Verify: `cargo test -p cobolt-forms --features render --test layers_model_091`; AC1 `diff -r` of all 97 saved forms clean.

- [x] **T3 — The engine draws layers** (R9–R12, R15; AC5, AC6, AC8 render) — `8053bf6`, 1.90.29
  - Files: `cobolt-forms/src/{render.rs,snapshot.rs,toolbar.rs}`; `tests/layers_engine_091.rs`.
  - Layer-major order, layer backgrounds between layers, hidden layers, per-layer deferred passes, Tab order — on all three paint paths.
  - Verify: `cargo test -p cobolt-forms --features render --test layers_engine_091` and the parity guard `engine_reference_form_parity_static_vs_faces`.

- [x] **T4 — A layer is cut at the form's edge** (R13, R14; AC7) — `9ecdf91`, 1.90.30
  - Files: `cobolt-forms/src/render.rs`; `tests/a_rounded_window_ends_at_its_arc.rs`.
  - 62 types × 3 paths; Q31 lists the seven types that cannot clip to an arc.
  - Verify: `cargo test -p cobolt-forms --features render --test a_rounded_window_ends_at_its_arc`.

- [x] **T5 — Only the base is laid out** (R13, R21 engine) — `2a03dc8`, 1.90.31
  - Files: `cobolt-forms/src/layout/{mod.rs,apply.rs}`; `tests/layers_layout_091.rs`.
  - Verify: `cargo test -p cobolt-forms --features render --test layers_layout_091`.

- [x] **T6 — The layers hold the pointer** (R17–R20; AC9) — `52a9239`, 1.90.32
  - Files: `cobolt-forms/src/render.rs` (`pointer_claim`, `owns_pointer_at`, blockers, event gate, opaque shields).
  - Verify: `cargo test -p cobolt-forms --features render` (the AC9 cases, with three layers).

- [x] **T7 — A program shows and hides a layer, on every host** (R33–R38, R42, R55 run time; AC12, AC13 golden, AC14, AC27 build part) — `e6e355a`, 1.90.33
  - Files: `cobolt-form-host/src/{state.rs,seeding.rs,host.rs,shell.rs}`, `cobolt-runtime/src/interpreter.rs`, `cobolt-semantic/src/{lib.rs,resolver.rs}`, `cobolt-compiler`, `cobolt-cli`, `cobolt-project-tools`, `cobolt-ide/src/{external_crates_service.rs,panels/editor.rs}`.
  - Verify: `cargo test -p cobolt-form-host`; `-p cobolt-semantic`; `-p cobolt-runtime --test test_layer_writes`; `-p cobolt-cli --test layers_hosts_091` and, once, `-- --ignored` for the built binary.

- [x] **T8 — Fix: `LaidOutState` did not forward `layer_visible`** (R35) — `222469b`, 1.90.34
  - Verify: `cargo test -p cobolt-forms --features render --test layers_layout_091`.

## Slice 4 — the tab bar and the Non-Visuals tab in the designer

- [x] **T9 — The tab bar's state, strings and painting** (R23, R43, R46, R60–R62; AC16 colours) — worktree, **uncommitted**
  - Files: `cobolt-ide/src/panels/layer_tabs.rs` (new), `panels/mod.rs`, `i18n.rs` (15 `Tr` keys × 6 languages + `layer_strings_091`).
  - `LayerTabs` rules (select / toggle shown / retarget / reconcile / view), `bar_layout`, `drop_position`, `show_bar`, the same blue/white on every theme, the red ✕ on a white chip when its tab is active.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layer_tabs` (13 pass) and `layer_strings_091`.

- [x] **T10 — The canvas answers for the active tab** (R29, R44, R56; compile-checked only) — worktree, **uncommitted**
  - Files: `cobolt-ide/src/panels/designer.rs`: `tabs`, `DesignerState{anim,tabs}`, `TabView`, `Backdrop.layers`, `non_visuals_view`, `canvas_size`, `active_tab_ids`/`active_tab_rects`, the active-tab filter at `hit_top_id`, hover, Select All, lasso, `splitter_division_at`, `tab_strip_hit`.
  - Verify: `cargo check -p cobolt-ide --bin cobolt-ide --tests` clean; behaviour is proved by T15.

- [x] **T11 — Register the bar and apply its actions** (R23, R24, R25, R26 select, R43, R44 retention, R60–R62; AC11 bar, AC16, AC19 bar, AC29, AC30)
  - Files: `designer.rs` (`DesignerPanel::show`).
  - `egui::Panel::bottom(Id::new(("layer-tabs", ai_pane_id()))).exact_size(BAR_H).frame(NONE)` before `canvas_max_h`; `tabs.reconcile(&form)` every frame; `apply_tab_actions` — select (R60/R61, no dirty flag, no undo), toggle shown (R62, active tab unchanged), drop selected controls that leave the active tab (R44).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::bar` — the bar is 32 px on a form with no layers and the window size is unchanged with 64 tabs; AC29/AC30 sequences leave `modified` false and the undo stack empty.

- [x] **T12 — The form is only resized from `Form`** (R30, R52, R57; AC19 resize, AC24 drag)
  - Files: `designer.rs` (`handle_drag`, the form outline, `Width`/`Height` editability).
  - Form-edge grips only on `Form`; on `Non-Visuals` no control handles and no move drag, and the form outline is hidden; cards are selectable only.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::gating` — a drag on a card changes no cell and no `X`/`Y`; grips absent on a layer and on `Non-Visuals`.

- [x] **T13 — The toolbox chooses the tab, and disables what the tab cannot take** (R28, R48, R58, R59; AC21, AC28)
  - Files: `panels/toolbox.rs`, `app.rs`, `designer.rs`; `i18n.rs` already carries `toolbox_visual_disabled_hint` and `layer_paste_refused`.
  - `ToolboxAction.pressed` set where `set_payload` is called; a non-visual press selects `Non-Visuals` without touching any `Visible`; every visual entry (user controls included) is drawn greyed and takes no press, click or drag while `Non-Visuals` is active; paste is disabled when the clipboard holds any visual control (all or nothing, Q29) and refused with a message if one arrives another way.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::toolbox` — the AC28 sequence on `Layer 1`, `Form` and `Non-Visuals`; clipboard of a Button, of a Button + Timer, of a Timer only.

- [x] **T14 — A new control lands in the active tab; the preview shows the layers** (R28, R47, R54; AC20, AC26)
  - Files: `designer.rs` (`add_control`), `app.rs` / the preview's `PreviewState` and `Backdrop` (layers all shown, as the snapshot does).
  - `add_control` sets `Control.layer` to the active layer (none on `Form`, none for a non-visual one); non-visual controls already in a form sit in the grid with their `X`/`Y` untouched.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::placement` — a test walks `ControlType::ALL` (non-visual ⇔ `is_non_visual()`); `cargo test -p cobolt-forms --features render --test every_example_form_saves_unchanged_091` — AC26's before/after dump still clean.

- [x] **T15 — Selection, lasso, Select All and the grid, per tab** (R29, R44, R45, R49–R52; AC17, AC18, AC22, AC23, AC24)
  - Files: `designer.rs` tests (`event_editor_drag_tests::frame` harness); any existing designer test that clicked a non-visual control on the `Form` tab is updated.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::selection` — click / Ctrl-click / drag / double-click / lasso / Cmd+A on base, `Layer 1`, `Layer 2`; 1, 5, 6, 11 cards give the cells of AC22; deleting the third moves the rest up; scrambled types and names read A–Z (AC23) under all six `Language::ALL`.

- [x] **T16 — Slice 4 gate and commit** (AC11, AC16, AC19)
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --tests` (its only red is the pre-existing `docs_embed::every_document_ships_in_every_language`); `cargo test -p cobolt-forms --features render`.
  - Do: bump `z`, CHANGELOG, commit "Spec 091 slice 4: the tab bar and the Non-Visuals tab". Nothing else in the commit.

## Slice 5 — layer operations and the inspector

- [x] **T17 — Add, rename, re-stack, with undo** (R7, R26, R32, R6; AC2 UI, AC3, AC11 re-stack, AC30 `+`)
  - Files: `designer.rs` (`Cmd::SetLayers { before, after }`, precedent `SetBreakpoints`), `layer_tabs.rs` (actions), `i18n.rs` if a key is missing.
  - `+` adds `Layer-N` and **selects it** (Q30); refused at the 65th with `layer_limit_reached`; rename refused with `layer_name_refused` (reserved, collision, bad characters) and `tabs` retargeted on success; drag re-stack through `Form::move_layer`; each is one undo step and redoes.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::ops` — 64 added, 65th refused, window size unchanged; `Form`/`Non-Visuals` cannot be dragged or dropped to; undo/redo restores names, order and `tabs`.

- [x] **T18 — A layer's own properties in the inspector** (R15, R16, R24; AC8 UI)
  - Files: `panels/properties.rs`, `designer.rs` (a `layer_props` channel apart from `set_property`, which ignores unknown ids).
  - Layer view when a layer tab is active: Name, background colour, gradient, image + mode, Transparency; no `CornerRadius`, no window property, no `Visible` row; a new layer is fully transparent; edits are undo steps.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::layer_props`.

- [x] **T19 — A control's `Layer`, and moving controls between layers** (R3, R8, R21, R31, R44; AC4, AC10, AC17 move)
  - Files: `designer.rs` (`apply_structural_prop`, `property_names_for` — `Layer` is a struct field, not in `properties`), `panels/properties.rs`, the control context menu.
  - Inspector `Layer` choice and "Move to layer" as one undo step; Dock → `None` and Anchor → default when moving from the base, restored by undo; Dock/Anchor/layout rows hidden for layer controls; children follow their container and cannot be moved alone; the list never offers `Non-Visuals`; moved controls leave the selection.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::move_to_layer`.

- [x] **T20 — Non-visual controls: inspector, delete, paste, duplicate** (R5, R47, R48, R53, R54; AC2 collisions, AC20, AC21 paste/duplicate, AC25)
  - Files: `designer.rs`, `panels/properties.rs`.
  - No `Layer` row and no `X`/`Y`/`Width`/`Height` for a non-visual control; Delete / Backspace / context-menu Delete go through `DeleteControl` (confirmation when it carries code, recycle bin, one undo step), and a procedure that only mentions the control stays and is reported; paste and duplicate of a non-visual control select `Non-Visuals`; a colliding name gets the next free one.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::non_visual` — delete then undo brings back the control and its handler; the Output panel carries the report.

- [x] **T21 — The problems a file can carry are reported, not repaired** (R5, R40; AC2 load, AC13)
  - Files: `app.rs` / the form-open path, `cobolt-forms/src/model.rs` (`name_collisions`, `unknown_layer_refs` already exist).
  - A `.cfrm` with a name collision or an undefined layer opens with every control kept (the latter drawn in the base) and one Output line per problem.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::load_report`; `cargo test -p cobolt-forms --features render --test layers_model_091`.

- [x] **T22 — Slice 5 gate and commit**
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide --tests`; `cargo test -p cobolt-forms --features render`. Bump `z`, CHANGELOG, commit.

## Slice 6 — deleting a layer

- [ ] **T23 — The confirmation window** (R27, R63; AC31)
  - Files: `designer.rs` (`pending_layer_delete`, added to `has_blocking_modal`), `layer_tabs.rs`; keys `layer_delete_title`/`layer_delete_body` exist.
  - The ✕ calls `tabs.select_layer` (R60/R61) and opens a modal that names the layer and counts controls (children included) and handlers (`ctrl.events` with `has_code()`) — also for an empty layer (0 and 0). Fixed-size on the `error_window` pattern: no `available_width()`, no self-resize. Cancel leaves everything and the layer active; nothing behind reacts.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::delete_modal` — the window's size is identical before and after its text is set in each of `Language::ALL`, and over several frames.

- [ ] **T24 — The deletion, and its undo** (R64, R65; AC32, AC33)
  - Files: `designer.rs` (one `Batch` of `DeleteControl` for every control, children included, then `SetLayers`; a snapshot of `data_bindings`, which `recycle_control` prunes and its reverse does not restore).
  - `Form` is active afterwards (Q27); a procedure bound to none of them but mentioning one stays and is reported; the next generation has no layer.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide layers_091::delete_layer` — one undo gives back layer, position, properties, controls (name, properties, `ZOrder`, container link), handlers, bindings, and the regenerated COBOL equals the pre-delete text; the undo stack grew by exactly one step.

- [ ] **T25 — Slice 6 gate and commit**
  - Verify: as T22.

## Slice 7 — documentation, knowledge base, measurement

- [ ] **T26 — System KB** (steering `tech.md`: behaviours, controls, properties, methods change ⇒ KB in the same change)
  - Files: `cobolt-compiler/src/lib.rs` (the property/method/event tables and `CFRM_PROSE`: `Layer` object, its properties, the `Layer` control property, `Non-Visuals`), `assets/knowledge/chunked.data`.
  - Run: `cargo run -p cobolt-ide --example build_chunked_kb` (needs the `multilingual-e5-small` model).
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide prebuilt_chunked_kb_matches_the_published_documentation` green; `git diff --stat assets/knowledge/chunked.data` shows it changed.

- [ ] **T27 — Developer's Guide** (GOLDEN RULE #3; audience: Fujitsu PowerCOBOL / isCOBOL developers)
  - Files: `docs/developers-guide-en.md` only — §7 (the designer's tab bar and the Non-Visuals tab), §11 (the controls' common properties: `Layer`), §22 (layers at run time: `LAYER-1::Visible`, starts hidden, stays shown). COBOL examples and prose, no Rust; a mermaid diagram of the pointer's path through layers; `📷 Screenshot needed — <name>.png` placeholders; Notes and ⚠️ Caveats (Q31's seven types, `Visible` never saved, IDE Check does not run the receiver check).
  - Then delete every `docs/developers-guide-<lang>.md` the change invalidates (GOLDEN RULE #8) — `ls docs/developers-guide-*` first — and never the English one.
  - Verify: `cargo test -p cobolt-ide --bin cobolt-ide docs_embed` — the two guards go red on a deleted translation, which is the intended signal; no other docs test regresses.

- [ ] **T28 — The cost of layers, measured** (AC15; GOLDEN RULE #7)
  - Files: `cobolt-forms/tests/` (an `--ignored` bench) or `cobolt-bench`, whichever the existing frame benches use.
  - 64 layers × 50 controls against the same 3,200 in the base: frame time and hit-test time, printed in one summary block with the cases named.
  - Verify: `cargo test -p cobolt-forms --features render --release --test layers_bench_091 -- --ignored --nocapture`; the numbers go in the CHANGELOG entry; a budget is set by the plan, not invented here.

## Finalize

- [ ] **T29 — Whole-suite run and the foreign-code sweep** (all AC)
  - Verify, one crate at a time: `cargo test -p cobolt-forms --features render`; `-p cobolt-form-host`; `-p cobolt-semantic`; `-p cobolt-codegen`; `-p cobolt-project-tools`; `-p cobolt-runtime --lib` and the nearest test files; `-p cobolt-compiler`; `-p cobolt-cli`; `cargo test -p cobolt-ide --bin cobolt-ide --tests`. Read every `test result:` line, `--no-fail-fast`. Known and not ours: the translation guard, live-network and `libsqlite3-sys` failures.
  - Sweep the tree for untracked non-Rust program source (CLAUDE.md, PRIME DIRECTIVE) before the final commit; move, never delete.

- [ ] **T30 — Close the spec**
  - Files: `spec.md` (every AC ticked, Status → implemented), this file, `CHANGELOG.md` (one entry for 091 as a whole with the AC15 numbers), `version.rs` (the commit's own `z`).
  - Stop there: no merge to `main`, no push, no forum post (rules #1, #4b, #5). The operator decides; the open items stay listed — Q31's default, IDE Check's receiver check, and the unrelated `leaderboard_prototype` example (task_7698cf21).

## Coverage: every acceptance criterion has a task whose verify line proves it

| AC | Tasks | AC | Tasks |
|---|---|---|---|
| AC1 | T1, T2, T14 | AC17 | T10, T15, T19 |
| AC2 | T2, T17, T20, T21 | AC18 | T15 |
| AC3 | T2, T17 | AC19 | T11, T12, T16 |
| AC4 | T2, T19 | AC20 | T14, T20 |
| AC5 | T3 | AC21 | T13, T20 |
| AC6 | T3 | AC22 | T2 (pure grid), T15 |
| AC7 | T4 | AC23 | T2 (order), T15 |
| AC8 | T3, T18 | AC24 | T12, T15 |
| AC9 | T6 | AC25 | T20 |
| AC10 | T5, T19 | AC26 | T14 (corpus dump) |
| AC11 | T11, T16, T17 | AC27 | T7, T11 (Non-Visuals not a layer), T17 (name refused) |
| AC12 | T7, T8 | AC28 | T13 |
| AC13 | T2, T7, T21 | AC29 | T11 |
| AC14 | T7 | AC30 | T11 |
| AC15 | T28 | AC31 | T23 |
| AC16 | T9, T11 | AC32 | T24 |
| | | AC33 | T24 |

Done when: every AC above is ticked in `spec.md`; the T29 runs are green but for the listed environmental reds; the guide and the System KB carry the feature; the commits are one per slice, each a feature commit and nothing else; nothing is merged or pushed unasked.
