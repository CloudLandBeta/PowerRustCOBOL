# Tasks — Form container controls (real containment & reparenting)

- **Status:** draft → in progress
- **Plan:** ./plan.md   **Date:** 2026-06-20

Ordered, small, independently-verifiable. Each keeps the workspace green where
possible. Pure (non-egui) logic is extracted into testable helpers; egui-bound
behaviour is verified by the manual launch checks in T13.

- [x] **T1 — Model: parent/tab fields, container helpers, new props** (R1, R2, R5, R6, R19)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: add `parent: Option<String>` and `tab: Option<u32>` to `Control`
    (serde-default). Add `Control::is_container()` (`GroupBox|Panel|TabControl`)
    and `content_rect(&self) -> Rect` (interior, inset for GroupBox caption / Panel
    border / TabControl strip). Add container props `BorderRadius` (Int 0) and
    `AutoScroll` (Bool false) to GroupBox/Panel/TabControl; keep `Opacity` (Int
    100). Ids stay unique form-wide (R19 — no change needed; assert in test).
  - Verify: `cargo test -p cobolt-forms --lib` green; new unit test asserts the
    three containers expose `BorderRadius`/`AutoScroll`/`Opacity`, non-containers
    don't gain them, and `is_container()` is correct.

- [x] **T2 — Serialization: flat↔tree, tab/props, round-trip + migration** (R16, R3)
  - Files: `crates/cobolt-forms/src/xml.rs` (+ helpers in `model.rs`)
  - Do: on **save**, build the `<Children>` tree from `parent` links (top-level =
    `parent None`) and write each child's `tab` + the new props; on **load**,
    flatten the tree into `form.controls` setting `parent`/`tab`. Read legacy
    `Panel.Scrollable` as a fallback for `AutoScroll`. Older `.cfrm` (no nesting)
    load as all-top-level unchanged.
  - Verify: `cargo test -p cobolt-forms` green; round-trip test builds
    `Panel⊃GroupBox⊃TabControl⊃Panel⊃TextBox` (with tabs + new props), saves,
    loads, asserts identical tree/parent/tab/props; legacy-`Scrollable` migration
    test.

- [x] **T3 — Codegen: nested controls emitted (regression)** (R18)
  - Files: `crates/cobolt-codegen/tests/` (new test; no functional change expected)
  - Do: assert `generate(form)` for a form whose only control is nested two levels
    deep includes that control (banner + regenerate contract untouched).
  - Verify: `cargo test -p cobolt-codegen` green.

- [x] **T4 — Renderer: border-radius frames, opacity via alpha_mul, clip helper** (R4, R5, R6b)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: container frames/backgrounds honour `BorderRadius`; fold `Opacity` (0–100)
    into the existing `alpha_mul` thread in `draw_control`/`draw_chart_preview`;
    expose a small shared `content_clip_rect`/opacity-compose helper for the
    designer + runtime walks. (Rounded child-clip approximated: rectangular clip +
    rounded frame painted over corners — see plan §5.)
  - Verify: `cargo build -p cobolt-forms`; `cargo test -p cobolt-forms`; unit test
    for the opacity-compose helper (chain of opacities multiplies correctly).

- [x] **T5 — Designer: tree-order draw with clip/visibility/opacity context** (R11, R4, R6b)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: replace the flat z-sorted draw with a **parent-order tree walk**, each
    level sorted by sibling `z_order`, carrying (clip-rect, visible, opacity,
    scroll) context; skip controls hidden by inactive tab / clipped out. Un-nested
    forms must render exactly as before.
  - Verify: `cargo build -p cobolt-ide`; extract the order/visibility computation
    into a pure helper with a unit test (flat form → same order as old z-sort;
    nested form → parents before children, clip composed). Manual: existing forms
    look unchanged (T13).

- [x] **T6 — Designer: reparent-on-drop + Reparent command + cycle guard** (R7, R8, R9, R10, R15) — drag-highlight (R12) deferred
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: add `Cmd::Reparent { id, old_parent, old_tab, new_parent, new_tab }`
    (apply/undo). On drop, resolve the target from the cursor point via a pure
    `resolve_drop_target(point, &controls, active_tabs) -> Target` =
    {Form | Container(id) | SiblingOf(id→its parent)}, honouring visible content
    area only (reject clipped/scrolled-out/inactive-tab). Absolute coords
    unchanged (R15). Highlight the target container during the drag (R12). Reject
    container-into-own-descendant (cycle guard).
  - Verify: `cargo build -p cobolt-ide`; unit tests on `resolve_drop_target` for
    R7 (canvas→Form), R8 (into container / same container), R9 (clipped/inactive →
    rejected), R10 (over non-container → sibling's parent); cycle-guard test.
    Manual drag in/out/between/over-sibling (T13).

- [x] **T7 — Designer: move-with-parent, cascade delete, per-parent z-order** (R2, R13, R14) — cascade undo is per-control (not one batch step)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: when a container moves, fold its whole subtree into `MoveMany` (same
    delta). Delete cascades to descendants as one undoable group. Z-order
    reordering scoped to siblings within a parent.
  - Verify: `cargo build -p cobolt-ide`; unit tests for `collect_descendants(id)`
    (cascade set) and the subtree-move delta set. Manual: move/delete a populated
    container + undo (T13).

- [x] **T8 — TabControl: per-tab grouping + tab selection + active-tab visibility** (R3, R11)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`, `crates/cobolt-forms/src/paint.rs`
  - Do: paint the full tab strip; clicking a tab sets the design-time active page
    (seeded from `SelectedTab`); only children with `tab == active` are
    drawn/selectable/hit-tested; dropping into a TabControl assigns the active
    `tab`.
  - Verify: `cargo build -p cobolt-ide`; unit test for the active-tab visibility
    filter. Manual: two tabs with different controls; switching shows/hides (T13).

- [~] **T9 — Auto-scroll behaviour (clip vs scroll)** (R6) — DEFERRED: `AutoScroll` prop + editable + clip-when-off works; scrollbars/scroll-offset plumbing across draw+hit+reparent not yet wired
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: per-container scroll offset state; when `AutoScroll` on and children
    overflow `content_rect`, translate children by `-offset` and show scrollbars;
    when off, clip with no scroll. Point→content transform shared by draw + hit.
  - Verify: `cargo build -p cobolt-ide`; unit test for the point→content transform
    + overflow/extent calc. Manual: overflow scrolls vs clips per the toggle (T13).

- [x] **T10 — Properties pane: radius / auto-scroll / opacity rows** (R5, R6, R6b, R19) — Opacity row already existed (now functional); tab selector with T8
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `crates/cobolt-ide/src/i18n.rs`
  - Do: for the three containers add rows for `BorderRadius`, `AutoScroll`,
    `Opacity`, and a TabControl tab selector for choosing the page to edit. All
    labels via new `Tr` keys.
  - Verify: `cargo build -p cobolt-ide`; editing each prop updates the control;
    `cargo test -p cobolt-ide i18n` green.

- [x] **T11 — Runtime parity: context-aware render + input** (R3, R4, R6b, R17) — IDE preview now tree-order + clip + tab-visibility + ancestor-opacity; standalone-binary render path not audited; auto-scroll (R6) deferred with T9
  - Files: `crates/cobolt-ide/src/form_runtime.rs`
  - Do: add `parent`/`tab` to `CtrlMeta`; apply the same clip / active-tab /
    opacity / scroll context as the designer (shared helpers where practical) in
    both render and input/hit handling, so a running form matches the designer.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide`. Manual: Run a
    nested form — clipping, tab switching, opacity, scroll all behave as in the
    designer (T13).

- [x] **T12 — Docs & i18n** (R21) — English guide "Containers and nesting" added; container property-pane labels use inline literals to match the existing pane convention (i18n test green); a full `Tr` pass for the pane is pre-existing debt (R20 deferred)
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: document containers, nesting/any-combination, the reparent/drop rules,
    border-radius clipping, per-tab grouping, auto-scroll, and opacity; confirm all
    new `Tr` keys exist in **all six** languages (EN/ES/PT/JA/ZH/FR). English guide
    only — translations untouched.
  - Verify: `cargo test -p cobolt-ide i18n` (no empty translations); guide section
    renders.

- [x] **T13 — Finalize** (all ACs) — version 1.27.4 (fix, per pre-prod directive) + CHANGELOG; `cargo test --workspace` green (71 binaries). AC1–AC10 covered by automated `containers`/forms/codegen tests + manual designer/preview checks; AC7 scrollbars deferred with T9
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: version + CHANGELOG bump (**feature → 1.28.0** per `tech.md`, *unless the
    operator confirms patch* per the standing fix directive — confirm before
    commit). Full `cargo test --workspace`. Walk every acceptance criterion AC1–
    AC10 manually in `cargo run -p cobolt-ide` (build the nested demo form; drag
    in/out/between/over-sibling; tabs; radius/opacity/auto-scroll; save→reopen;
    Run for parity; cascade-delete + undo).
  - Verify: `cargo test --workspace` green; AC1–AC10 observed; no "cobolt" in new
    user-facing text.

## Acceptance-criteria coverage
- AC1 → T1, T2, T5, T7, T11 · AC2 → T8, T11 · AC3 → T4, T5, T11 ·
  AC4 → T6 · AC5 → T6 · AC6 → T6 · AC7 → T9, T11 · AC7b → T4, T10, T11 ·
  AC8 → T7 · AC9 → T2, T3 · AC10 → T12

## Done criteria
All spec.md acceptance criteria checked, `cargo test --workspace` green, English
guide + six-language i18n updated, and the change committed per the operator's
rules (fix-vs-feature classification confirmed; do **not** commit/push/publish
unless the operator asks).
