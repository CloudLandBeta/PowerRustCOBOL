# Plan — Form container controls (real containment & reparenting)

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-20

## 1. Approach

The designer edits a **flat `form.controls` list** with id/index commands
(`MoveControl`, `ResizeControl`, `ReorderControl`, `SetZOrder`, `MoveMany`,
`Add/DeleteControl`). Rather than rewrite that machinery onto the `children` tree,
we **keep the flat list and add a parent link**, deriving nesting behaviour from
it. The `children` tree remains the on-disk / runtime / codegen shape (both the
runtime `collect_controls` and codegen `collect_all_controls` already recurse it),
so we **convert flat↔tree only at the `.cfrm` boundary**. Coordinates stay
**absolute** everywhere; "relative to the container" (R2) is a *behaviour*
(move-with-parent, clip, scroll) computed from absolute rects + the parent chain —
this preserves all existing flat command math and makes reparenting position-
preserving (R15) for free.

- **Model (R1, R3):** add `parent: Option<String>` and `tab: Option<u32>` to
  `Control`. `parent` = enclosing container id (None = form). `tab` = which
  TabControl page a child belongs to (only meaningful when the parent is a
  `TabControl`). `children` stays empty in the live editing model and is
  (re)built at save.
- **Containers & nesting (R1):** a control type is a *container* iff it is
  `GroupBox | Panel | TabControl`; any control (incl. containers) may set its
  `parent` to any container id → arbitrary depth / any combination. A
  `content_rect(ctrl)` helper returns each container's interior (inset for the
  GroupBox caption band / Panel border / TabControl strip).
- **Reparent on drop (R7–R10, R15):** after a move/drop, resolve the drop target
  from the cursor point: the **topmost visible container whose `content_rect`
  (after clipping + active-tab + scroll) contains the point** → reparent into it
  (R8); if the point is over a **non-container control**, adopt *that control's*
  parent (R10); if over bare canvas, parent = form (R7). A drop over a
  clipped/scrolled-out region or an inactive tab page is **not** a target (R9). A
  new `Reparent { id, old_parent, old_tab, new_parent, new_tab }` command makes it
  undoable; absolute coords are unchanged so the on-screen position is preserved
  (R15). Reparenting that would put a container inside its own descendant is
  rejected (cycle guard, Q5).
- **Move-with-parent (R2):** when a container moves, its whole subtree moves by
  the same delta — fold descendants into the existing `MoveMany` command.
- **Clipping + radius (R4, R5):** rendering walks the tree with a **clip stack**:
  each child is drawn through `painter.with_clip_rect(intersection of ancestor
  content rects)`. egui clip rects are **rectangular**, so the configurable
  `BorderRadius` is applied to the container's drawn frame/background and the
  rounded frame is painted *over* the child corners to mask corner overflow
  (pixel-exact rounded child-clipping is out of egui's built-in reach — see
  Risks). `BorderRadius` is a new int prop on the three containers.
- **Auto-scroll (R6):** a new `AutoScroll` bool prop (default off) on each
  container, superseding `Panel`'s existing `Scrollable`. When on and the child
  bounding box exceeds the content rect, a scroll offset is kept per container
  (`HashMap<String, Vec2>` in the designer; egui `ScrollArea`-style at run time);
  children translate by `-offset` and clip to the content rect; when off, content
  is clipped with no scrolling.
- **Opacity (R6b):** read the container's `Opacity` (0–100) and fold it (×
  ancestor opacities) into the `alpha_mul` already threaded through
  `draw_control` / `draw_chart_preview`, so the container's frame **and its child
  subtree** fade. This fixes the currently-dead `Opacity` property.
- **TabControl pages (R3):** the active page = `SelectedTab`. Only children with
  `tab == SelectedTab` are visible/interactive. At design time, clicking a tab in
  the strip selects the page being edited (stored in designer state, seeded from
  `SelectedTab`).
- **Designer render/hit-test (R11, R12, R14):** the canvas draw loop and the
  selection/drag hit-tests change from "flat list sorted by z" to a **tree walk
  in parent order**, each level sorted by sibling `z_order`, carrying
  (clip, visible, opacity, scroll) context; hidden (inactive tab / clipped)
  controls are skipped for both draw and hit-test. A drag that would reparent
  highlights the target container (R12).
- **Lifecycle (R13):** delete cascades to descendants as one undoable
  `DeleteControl` group.
- **Persistence (R16):** on save, build the `<Children>` tree from `parent`
  links and write each child's `tab`; on load, flatten the tree to `form.controls`
  and set `parent`/`tab`. Format stays the nested `.cfrm` already supported.
- **Runtime (R17):** `form_runtime` keeps the tree; `CtrlMeta` gains
  `parent`/`tab`, and the render + input loops apply the same clip/visible/
  opacity/scroll context as the designer (one shared helper where practical).
- **Codegen (R18):** already recurses children — add a regression test proving a
  nested control is emitted; banner/regenerate contract untouched.
- **Property access (R19):** ids stay unique form-wide (the existing
  `next_unique_id`), so `control::property` / `INVOKE` / event bindings are
  unaffected — a nested control is still addressed by id.

## 2. Affected crates / files
- `crates/cobolt-forms/src/model.rs` — `Control.parent` + `Control.tab` fields;
  `is_container()` + `content_rect()` helpers; `BorderRadius`, `AutoScroll` props
  on GroupBox/Panel/TabControl (+ keep `Opacity`).
- `crates/cobolt-forms/src/xml.rs` — flat↔tree at save/load; write/read
  `tab`/`BorderRadius`/`AutoScroll`; (parent derived from tree position).
- `crates/cobolt-forms/src/paint.rs` — container frames honour `BorderRadius`;
  TabControl strip + active-page content; opacity via `alpha_mul`; helpers shared
  with the render walk.
- `crates/cobolt-ide/src/panels/designer.rs` — `Reparent` command + cascade
  delete; tree-order draw with clip/visible/opacity/scroll; reparent-on-drop
  resolution + drop-target highlight; per-container scroll state; active-tab
  state; move-with-parent in `MoveMany`; cycle guard.
- `crates/cobolt-ide/src/panels/properties.rs` — `BorderRadius`, `AutoScroll`,
  `Opacity` rows for the three containers; TabControl tab selector for editing.
- `crates/cobolt-ide/src/form_runtime.rs` — `CtrlMeta.parent/tab`; context-aware
  render + hit/input (clip, active tab, opacity, scroll).
- `crates/cobolt-codegen/src/lib.rs` — no functional change; add nested-emit test.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` keys ×6 (drop hint, "Border radius",
  "Auto-scroll", "Opacity", tab-edit label).
- `docs/developers-guide-en.md` — containers, nesting, reparent/drop rules,
  radius/clip, per-tab grouping, auto-scroll, opacity.
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — version bump (see §7).

## 3. Data / model changes
- **`Control`** gains `parent: Option<String>` (default `None`) and
  `tab: Option<u32>` (default `None`). Serde-default both so older `.cfrm` load
  unchanged (everything parent-less = top-level).
- **New props** (containers only): `BorderRadius` (Int, default 0), `AutoScroll`
  (Bool, default false). `Panel.Scrollable` is migrated to `AutoScroll` (read old
  key as a fallback on load). `Opacity` already exists (Int 100) — now honoured.
- **`.cfrm`:** unchanged nesting via `<Children>`, plus `tab` and the new props as
  attributes/elements. **Back-compat:** existing forms have no nesting → load as
  all-top-level, render identically (radius 0, opacity 100, no scroll).
- **No COBOL/codegen contract change**; generated `.cbl` simply includes the
  nested controls (already supported).

## 4. Key decisions & alternatives
- **Flat list + `parent` link (not a tree-based designer).** — Why: the entire
  Cmd/selection/drag/align system is flat and id-addressed; a parent field adds
  nesting with minimal disruption. Rejected: refactor the designer onto
  `Control.children` (touches every command, high regression risk).
- **Absolute coordinates, relative *behaviour*.** — Why: no coordinate
  conversion on move/reparent; reparent preserves on-screen position (R15) for
  free; clip/scroll computed from rects. Rejected: store child coords relative to
  parent (conversion on every reparent/move, breaks flat align math).
- **Per-child `tab` index for TabControl pages (Q1).** — Why: pages are just a
  filter over children; no separate page-node type. Rejected: explicit tab-page
  container nodes (more model surface, extra nesting level).
- **`AutoScroll` prop, default off, supersedes `Scrollable` (Q3).** — Why: one
  clear name across all containers; default off keeps current look. Rejected: new
  separate prop leaving `Scrollable` dead.
- **`BorderRadius` default 0 (Q2); cascade-delete (Q4); cycle guard (Q5).** —
  match current visuals / RAD norms / correctness.
- **Container opacity fades the whole subtree.** — Why: matches the usual meaning
  of container opacity; single `alpha_mul` multiply down the chain. (Flagged in
  spec for the operator to veto in favour of frame-only.)

## 5. Risks & mitigations
- **Rounded child-clipping isn't native to egui** (clip rects are rectangular). →
  Clip children to the content **rect**; draw the container's rounded frame over
  child corners so corner overflow is masked; treat pixel-exact rounded clipping
  as out of scope (document). Revisit with a mask-texture approach only if needed.
- **Designer hit-test/selection rewrite** (flat→tree-order with visibility) could
  regress plain (un-nested) forms. → Keep the flat list; tree-order is just a
  sort+context pass; add tests that un-nested forms behave exactly as before.
- **Scroll + absolute coords interaction** (offset applied at draw *and*
  hit-test). → Centralise the point→content transform in one helper used by both
  draw and input so they can't drift.
- **`.cfrm` round-trip of parent/tab.** → Golden round-trip test (build a nested
  form, save, load, assert identical tree + tab + props).
- **Scope is large** (designer + renderer + runtime + serialization). → Phase it
  (see /tasks): model+serialize → render/clip/opacity → reparent → tabs →
  scroll → runtime parity → docs/finalize, keeping the build green per phase.
- **Version classification vs. operator directive.** → See §7; confirm at commit.

## 6. Test strategy
- **`cobolt-forms` (unit):** `parent`/`tab` defaults; `is_container`/
  `content_rect`; flat↔tree build/flatten is lossless; `.cfrm` round-trip of a
  `Panel⊃GroupBox⊃TabControl⊃Panel⊃TextBox` form (assert tree, tabs, new props);
  `Scrollable`→`AutoScroll` migration. Report counts.
- **`cobolt-codegen` (unit):** a nested control appears in `generate(form)`
  output (regression for R18).
- **`cobolt-ide` (unit, pure helpers):** drop-target resolution (point → form /
  container / sibling-parent / rejected-when-clipped/inactive-tab) per R7–R10;
  cycle-guard rejects container-into-descendant; cascade-delete collects the whole
  subtree; opacity/clip context composition down a chain; active-tab visibility
  filter. These are extracted as side-effect-free functions so they test without a
  live egui context.
- **Manual / visual (launch `cargo run -p cobolt-ide`):** build
  `Panel⊃GroupBox⊃TabControl`; drag a control in/out/between containers and over a
  sibling; switch tabs (design + Run); set BorderRadius (corner clip), Opacity
  (subtree fades), AutoScroll (overflow scrolls vs clips); Save→reopen identical;
  Run the form and confirm parity; delete a container (cascade) + undo.
- **i18n:** `cargo test -p cobolt-ide i18n` (×6) for the new keys.

## 7. Steering compliance
- [ ] **i18n:** new UI strings ("Border radius", "Auto-scroll", "Opacity",
      tab-edit label, drop hint) added to `i18n.rs` in all six languages
      (EN/ES/PT/JA/ZH/FR); referenced via `Tr`, no literals.
- [ ] **Generated-code:** banner + regenerate-on-Build/Run/Debug/Check preserved;
      codegen already emits nested controls (test added).
- [ ] **English dev guide** updated; translations untouched.
- [ ] **Fix vs feature:** by `tech.md` this is a **feature** → minor bump (1.27.x
      → 1.28.0) + CHANGELOG. **Conflict to confirm:** the operator's standing
      "treat all current changes as fixes" directive — resolve the version/commit
      classification with the operator before finalize (default to feature/minor
      unless told otherwise).
- [ ] **No "cobolt" in user-facing text; COBOL identifiers/source English.**
