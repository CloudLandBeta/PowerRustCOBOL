# Plan — Visual Repeating Groups (GroupBox arrays) — Phases 1–2

- **Status:** draft → approved
- **Spec:** ./spec.md   **Date:** 2026-06-21
- **Scope of this plan:** **Phases 1–2** (GroupBox visual properties +
  repeating-group metadata/UI/preview). Phases 3–5 (runtime cloning, indexed
  event dispatch, data binding) get their own plan once 1–2 land.

## 1. Approach

Everything in Phases 1–2 is **model + designer/properties + shared renderer**;
no runtime/codegen change yet. All new control state lives as **properties on the
`GroupBox`** in the existing `IndexMap<String,PropValue>`, which serialises to
`.cfrm` generically (`xml::write_control` iterates `ctrl.properties`), so no XML
schema work is needed (R9).

- **Phase 1 — visual props (R1–R6).** Add `HideCaption`, `HideBackground`,
  `BackgroundGradientEnabled`, `BackgroundGradientStartColor`,
  `BackgroundGradientEndColor`, `BackgroundGradientDirection` to `GroupBox`
  defaults (`BackgroundColor` and `BorderRadius` already exist from the universal
  block + spec 012). Teach the shared renderer (`cobolt-forms::paint::draw_control`):
  - `HideBackground` → skip the card fill+border, mirroring the chart
    `chart_frameless` guard already in place (R2).
  - `BackgroundGradientEnabled` → fill the (rounded) frame with a directional
    gradient mesh instead of the solid `BackgroundColor`, reusing the spec-013
    mesh approach; new helper `grad_dir_mesh(rect, start, end, dir)` covering
    Vertical / Horizontal / DiagonalDown / DiagonalUp / Radial (R5).
  - `HideCaption` → skip the GroupBox caption text (R1).
  - Children already clip to the rounded bounds via spec-012 `clip_rect` (R3) —
    no change needed.
- **Phase 2 — repeating-group metadata + UI (R7–R11).** Add the repeating-group
  property set to `GroupBox` defaults (all controls carry them; `IsRepeatingGroup`
  defaults false so existing forms are inert). The **properties pane** shows the
  new visual rows always (GroupBox), and a **Repeating Group** section only when
  `IsRepeatingGroup` is true. The **designer context menu** offers Set/Unset when
  a single `GroupBox` is selected (toggles `IsRepeatingGroup`; on set, seeds
  `ArrayName` with the control id if empty). The **designer** draws a small array
  **badge** on a repeating GroupBox and, when `PreviewItemCount > 1`, renders
  render-only **ghost clones** of the group + its descendant subtree at layout
  offsets (Vertical/Horizontal/Grid) — never added to the form model (R11).

## 2. Affected crates / files
- `crates/cobolt-forms/src/model.rs` — new `GroupBox` default props (Phase 1 +
  Phase 2 metadata); extend the existing container test.
- `crates/cobolt-forms/src/paint.rs` — `draw_control`: `HideBackground` skip,
  `HideCaption` skip, gradient fill; add `grad_dir_mesh` helper + unit tests.
- `crates/cobolt-ide/src/panels/properties.rs` — GroupBox visual rows +
  conditional "Repeating Group" section (all labels via `Tr`).
- `crates/cobolt-ide/src/panels/designer.rs` — context-menu Set/Unset, repeating
  badge, design-time preview clones; small helpers in `panels/containers.rs` if a
  layout-offset helper is warranted.
- `crates/cobolt-ide/src/i18n.rs` — new `Tr` keys (×6) for every new UI string.
- `docs/developers-guide-en.md` — document the new GroupBox properties + repeating
  groups (English only).
- `crates/cobolt-ide/src/version.rs` + `CHANGELOG.md` — version/notes (fix `z`,
  per standing directive).

## 3. Data / model changes
- **New `GroupBox` properties** (defaults): `HideCaption`=false,
  `HideBackground`=false, `BackgroundGradientEnabled`=false,
  `BackgroundGradientStartColor`="#F0F0F0", `BackgroundGradientEndColor`="#C8D0DC",
  `BackgroundGradientDirection`="Vertical"; **repeating:** `IsRepeatingGroup`=false,
  `ArrayName`="", `ItemCount`=0, `DataSource`="", `LayoutDirection`="Vertical",
  `ItemSpacing`=8, `ItemsPerRow`=1, `AutoScrollParent`=true, `CloneEvents`=true,
  `PreviewItemCount`=1.
- **`.cfrm`:** no schema change — properties serialise generically; old files load
  unchanged (missing props fall back to defaults via `get_prop`).
- **No AST/runtime/codegen change** in this plan.

## 4. Key decisions & alternatives
- **Reuse `BorderRadius`** (spec 012) for the corner radius — Why: avoids a
  duplicate `CornerRadius` property and the clip already honours it. Rejected:
  adding `CornerRadius` (Q1) — would split the source of truth.
- **All GroupBoxes carry repeating-group props** (inert when off) — Why: generic
  serialisation, no conditional schema, trivial toggle. Rejected: a separate
  optional struct — more plumbing, custom XML.
- **Preview clones are render-only** (drawn in the designer, not in the model) —
  Why: satisfies R11/AC12 without duplicating design-time controls; the template
  stays the single source. Rejected: materialising clone controls at design time —
  would pollute the model and selection/undo.
- **`ArrayName` empty ⇒ control id** at use sites; seeded to id on Set — Why:
  matches "default = GroupBox name" without going stale on rename.
- **Gradient via per-vertex mesh** reusing spec-013 approach — Why: smooth, no
  banding, already proven. Rejected: layered translucent rects (banding).

## 5. Risks & mitigations
- **Preview-clone coordinate/clip correctness** (children relative to a shifted
  group) → Risk of misplacement. Mitigation: compute clone offset as a single
  delta applied to the group and every descendant's absolute rect; reuse the same
  `clip_rect` intersection per clone; restrict v1 preview to **top-level**
  repeating GroupBoxes (skip nested) and cap at a sane max.
- **Radial/diagonal gradient over rounded rect** → mesh corners vs. radius.
  Mitigation: fill the gradient mesh and overdraw the rounded border stroke; for
  large radius accept minor corner squareness in the fill (same trade-off as
  existing glass meshes, noted at paint.rs:364).
- **i18n drift** (six languages) → Mitigation: add all keys in one edit; the
  existing `i18n` test enforces parity.
- **Property-pane clutter** → Mitigation: gate the Repeating Group section behind
  `IsRepeatingGroup`; group visual props under the existing Container header.

## 6. Test strategy
- **`cobolt-forms` (model)** — extend the container test: a `GroupBox` exposes the
  new visual + repeating props with the documented defaults; non-container
  controls do not. Reports each asserted default.
- **`cobolt-forms` (paint, `--features render`)** — unit-test `grad_dir_mesh`:
  vertex count + corner colours per direction (Vertical/Horizontal/Diagonal/
  Radial); assert distinct start/end vertices.
- **`cobolt-forms` (xml)** — round-trip a repeating `GroupBox` with non-default
  metadata + gradient props and assert equality after save/load.
- **`cobolt-ide` (i18n)** — existing parity test stays green with the new keys.
- **Manual/visual** — launch the IDE: drop a GroupBox, toggle HideCaption/
  HideBackground/gradient and confirm rendering; right-click → Set as Repeating
  Group → badge appears + section shows; set PreviewItemCount=3 and a layout and
  confirm ghost clones lay out without entering the model (undo/selection
  unaffected); reload the `.cfrm` and confirm persistence.

## 7. Steering compliance
- [x] i18n: all new UI strings added as `Tr` in 6 languages.
- [x] Generated-code banner + regenerate-on-action contract preserved (no codegen
  change in Phases 1–2).
- [x] English dev guide updated (translations untouched).
- [x] Fix vs feature: **fix** per standing pre-production directive → patch `z`
  bump + CHANGELOG; forum f=97 if/when published.
- [x] No "cobolt" in user-facing text; COBOL identifiers/source English.
