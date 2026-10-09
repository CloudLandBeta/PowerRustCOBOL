# Plan — Form layers and the Non-Visuals tab

- **Status:** approved (2026-10-09)
- **Spec:** ./spec.md   **Date:** 2026-10-09

> Written after slices 0–3 were already built, so §1 says what is **done** (with
> the commit) and what is **left**. The design below is the one the operator
> approved; the spec's §7 carries every question and its answer.

## 1. Approach

A form keeps its one **base** (named `Form`) and gains up to 64 **layers** above
it. A layer is a name, a backdrop (the form's own background properties, fully
transparent when new) and the controls that name it. Everything else follows from
three decisions: layers are drawn **by layer, then by `ZOrder`** (R10); only the
base is laid out (R21); and the designer edits **one tab at a time** — `Non-Visuals`,
`Form` or a layer — and treats the controls of every other tab as absent (R44).

| Slice | What it delivers | Requirements | State |
|---|---|---|---|
| 0 | Spec approved; the operator's answers applied; AC1 baseline walker | R2, R39 | **done** `cfc5f82` (1.90.27) |
| 1 | `Layer`, `Form.layers`, `Control.layer`, names, 64 cap, add / rename / re-stack / move, R8 container rule, `<Layer/>` and `layer=` (never `Visible`), `render_order_in`, pure `nv_grid` | R1–R8, R39, R40 | **done** `cd5eca8` (1.90.28) |
| 2a | Engine draws layers on all three paint paths: layer-major order, backgrounds between layers, hidden layers, per-layer deferred passes, Tab order | R9–R12, R15 | **done** `8053bf6` (1.90.29) |
| 2b | A layer's controls are cut at the form edge (62 types × 3 paths) | R13, R14 | **done** `9ecdf91` (1.90.30) |
| 2c | The layout engine leaves layer controls alone | R13, R21 | **done** `2a03dc8` (1.90.31) |
| 2d | The layers hold the pointer: `pointer_claim`, blockers, event gate, opaque shields | R17–R20 | **done** `52a9239` (1.90.32) |
| 3 | `Layer` object, hidden and sticky, one `initial_state` for the three hosts, analyser `known_layers`, interpreter write checks, editor completion | R33–R38, R42, R55 | **done** `e6e355a` (1.90.33), fix `222469b` (1.90.34) |
| 4 | The tab bar and the `Non-Visuals` tab in the designer | R23–R25, R28–R30, R43–R62 | **in flight** — strings, `layer_tabs.rs` and the canvas filters are in the worktree, uncommitted |
| 5 | Layer operations and the inspector | R6, R7, R15, R16, R21, R26, R31, R32, R53 | to do |
| 6 | Deleting a layer | R27, R63–R65 | to do |
| 7 | System KB, Developer's Guide, the AC15 bench | AC15; steering | to do |

### Slice 4 — what is left
1. Register the tab bar: `egui::Panel::bottom(Id::new(("layer-tabs", ai_pane_id()))).exact_size(BAR_H).frame(NONE)` just before `canvas_max_h` in `DesignerPanel::show`; `tabs.reconcile(&form)` every frame; `apply_tab_actions` (select, toggle shown, keep the selection inside the active tab — R44).
2. R30 / R52 / R57 gating in `handle_drag`: form-edge resize only on `Form`; no control handles and no move drag on `Non-Visuals`; the form outline hidden there.
3. Toolbox (R59): `pressed` on `ToolboxAction`, set where `set_payload` is called and applied in `app.rs`; a non-visual press selects `Non-Visuals`; the visual entries are **disabled** while it is active (hint `toolbox_visual_disabled_hint`); a paste holding any visual control is disabled there, all or nothing (Q29, `layer_paste_refused`).
4. New controls land in the active tab (R28): `add_control` sets `layer`; the IDE preview's `PreviewState` / `Backdrop` get the form's layers, all shown, as the snapshot does.
5. Tests with the `event_editor_drag_tests::frame` harness; any existing designer test that clicked a non-visual control on the `Form` tab is updated.
6. Full IDE suite, CHANGELOG, version, commit.

### Slice 5 — layer operations and the inspector
`Cmd::SetLayers { before, after }` (a whole-table snapshot; precedent `SetBreakpoints`): the `+` (selects the new layer, Q30), rename (retargets `tabs`; refused with `layer_name_refused`), re-stack by drag (`Form::move_layer`), layer properties, and the `Layer` choice and the context-menu "move to layer" for controls (R31; `Dock` → `None`, `Anchor` → default, undo restores both). Inspector: a layer view when a layer tab is active (a separate `layer_props` channel — `set_property` ignores unknown ids); `Layer` is a struct field handled in `apply_structural_prop` / `property_names_for`, not an entry of `properties`; `Dock`, `Anchor` and the layout rows are hidden for layer controls, and `X`/`Y`/`Width`/`Height` for non-visual ones. The loader reports a name collision or an undefined layer in the Output panel and repairs nothing (R5, R40).

### Slice 6 — deleting a layer (R63–R65)
The red ✕ calls `tabs.select_layer`, then sets `pending_layer_delete` (added to `has_blocking_modal`). The modal is fixed-size, on the `error_window` pattern, and names the layer with its control and handler counts (handlers = controls' `events` with `has_code()`). Confirm = one `Batch` of `DeleteControl` for every control (a container's children included) + `SetLayers` + a snapshot of `data_bindings`, because `recycle_control` prunes them and its reverse does not restore them. `Form` is active afterwards (Q27). A procedure bound to none of the deleted controls stays and is reported.

### Slice 7 — System KB, Guide, bench
KB constants in `cobolt-compiler/src/lib.rs` (and `CFRM_PROSE`), `assets/knowledge/chunked.data` regenerated, the freshness test green. `docs/developers-guide-en.md` only (§7, §11, §22). The AC15 bench: 64 layers × 50 controls against the same 3,200 in the base, printing frame time and hit-test time.

## 2. Affected crates / files

- `crates/cobolt-forms/src/{model.rs,xml.rs,containers.rs,nv_grid.rs,render.rs,snapshot.rs,toolbar.rs,lib.rs}`, `layout/{mod.rs,apply.rs}` — model, file format, grid, engine (done).
- `crates/cobolt-form-host/src/{state.rs,seeding.rs,host.rs,shell.rs}` — `initial_state`, `Layer` seeding, live layers (done).
- `crates/cobolt-runtime/src/interpreter.rs`, `crates/cobolt-semantic/src/{lib.rs,resolver.rs}`, `cobolt-compiler`, `cobolt-cli`, `cobolt-project-tools`, `cobolt-ide/src/external_crates_service.rs` — the `Layer` object and its checks (done).
- `crates/cobolt-ide/src/panels/layer_tabs.rs` (new), `panels/designer.rs`, `panels/toolbox.rs`, `panels/properties.rs`, `panels/editor.rs`, `app.rs` — the designer (slices 4–6).
- `crates/cobolt-ide/src/i18n.rs` — 15 keys × 6 languages already added; any further key goes in all six.
- `crates/cobolt-compiler/src/lib.rs`, `assets/knowledge/chunked.data` — System KB (slice 7).
- `docs/developers-guide-en.md` — §7, §11, §22 (slice 7); its translations are deleted, never edited (GOLDEN RULE #8).
- `crates/cobolt-forms/tests/{every_example_form_saves_unchanged_091,layers_model_091,layers_engine_091,layers_layout_091}.rs`, `a_rounded_window_ends_at_its_arc.rs`, `crates/cobolt-cli/tests/layers_hosts_091.rs`, `crates/cobolt-runtime/tests/test_layer_writes.rs` — tests (done); the designer's go in `designer.rs` and `layer_tabs.rs`.

## 3. Data / model changes

- `Form.layers: Vec<Layer>` — `Layer { name, backdrop: MenuPaneBackground }`, **no `visible` field**: `Visible` is run-time state and a designer aid, never saved (R35, Q26).
- `Control.layer: Option<String>` — root controls only; a child follows its container (R8). Non-visual controls carry none (R47).
- Names: layers and controls share one namespace, compared without regard to case; `Form` and `Non-Visuals` are reserved; at most 64 layers (`MAX_LAYERS`).
- `.cfrm`: a `<Layer name=… />` element per layer, in stack order, and a `layer=` attribute on a control, **each written only when used** — a form without layers saves exactly the text it saved before (AC1, measured on 97 forms).
- Run time: a layer is an interpreter object of class `Layer`, seeded hidden by `cobolt_form_host::state::initial_state` for all three hosts; `LAYER_PROPS` names what it has; booleans are spelled `true` / `false`.
- Nothing is migrated: the format change is additive.

## 4. Key decisions & alternatives

- **Z-order by layer, then `ZOrder`** (R10). *Rejected:* one global `ZOrder` — a layer could not be reasoned about as "above".
- **`Visible` is never saved; every layer starts hidden at run time** (R35, Q26). *Rejected:* saving the designer's checkbox as the starting value (Q2, reversed by the operator).
- **No new field on `RenderInput`.** Layers travel in `Backdrop`; `RenderInput` has about 118 literal sites.
- **One question asked once per frame for the pointer** (`pointer_claim`) instead of per-control checks (R17).
- **Layer controls are outside the layout tree** (R21): nothing from a layer takes room from the base's docks or raises its minimum size.
- **`Layer` as a struct field of the control, not a property** — a loaded control and a new one then read it the same way.
- **`Cmd::SetLayers` snapshots the whole table.** *Rejected:* one `Cmd` per operation — five undo paths for a list of at most 64 names.
- **The seven types that cannot clip to a rounded window's arc are measured, not cut** (Q31; default: leave, report in the Guide).
- **The Non-Visuals grid is derived, stores nothing** (R51, Q12): no `X`/`Y` is written back.

## 5. Risks & mitigations

- **Self-resizing windows** (the project's most repeated defect). The tab bar has a fixed height and scrolls instead of growing; the delete modal owns an explicit size and lays out from it → tests that render several frames with 64 tabs and in all six languages.
- **Hosts drifting apart** → `initial_state` is the one function; AC12 runs identically on `rcrun run-form`, an embedded child and a built binary.
- **A visual fix in one paint path only** → every engine test runs on the three paths; `engine_reference_form_parity_static_vs_faces` stays green.
- **Disk exhaustion looks like compile errors** → one crate at a time, test executables deleted between runs, stop below 2 GB free (the working protocol in the plan file).
- **Stale artifacts after a killed build** → `cargo clean -p <crate>`.
- **Concurrent sessions on the shared target/** → read `git diff` of every file before `git add`.
- **The tab filter is untested until slice 4's tests land** (T10 is compile-checked only) → T15 is written against the same harness as the existing designer tests before the slice is committed.

## 6. Test strategy

- Engine and model tests in `cobolt-forms` (`--features render`), one file per concern; each reports what it measured (GOLDEN RULE #7), and each was mutation-checked when written.
- Hosts: `layers_hosts_091.rs` (three hosts; the built binary is `#[ignore]`).
- Designer: `#[cfg(test)]` modules in `layer_tabs.rs` and `designer.rs`, driven by the `event_editor_drag_tests::frame` harness — selection, lasso, Select All, resize gating, toolbox state, undo, modal size across `Language::ALL`.
- The AC15 bench prints the numbers; the plan sets no budget until they exist.
- **No UI automation** — the operator looks at the screen (CLAUDE.md). Visual checks are left to them and listed in the final summary.

## 7. Steering compliance

- [x] i18n: all new UI strings in 6 languages (15 keys in; any later key in all six)
- [x] Generated-code banner + regenerate-on-action contract preserved (R41; `tests/golden/`)
- [ ] English dev guide updated (translations deleted, not edited) — T27
- [x] Fix vs feature: **feature** → `features` branch, `z` bump and CHANGELOG entry per commit; never the minor
- [x] No "cobolt" in user-facing text; COBOL identifiers English (`LAYER-1::Visible`, `Layer-N`)
- [ ] System KB constants updated and `chunked.data` regenerated — T26
