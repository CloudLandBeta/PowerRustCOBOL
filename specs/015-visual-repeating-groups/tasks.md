# Tasks — Visual Repeating Groups (GroupBox arrays) — Phases 1–2

- **Status:** Phases 1–2 **done** (2026-06-21)
- **Plan:** ./plan.md   **Date:** 2026-06-21

Phases 3–5 (runtime cloning, indexed event dispatch, data binding) are out of
scope here and tracked separately once 1–2 land.

- [x] **T1 — GroupBox visual + repeating props in the model** (R1,R2,R5,R6,R8,R9)
  - Files: `crates/cobolt-forms/src/model.rs`
  - Do: add the Phase-1 visual props (`HideCaption`, `HideBackground`,
    `BackgroundGradientEnabled/StartColor/EndColor/Direction`) and the Phase-2
    repeating props (`IsRepeatingGroup`, `ArrayName`, `ItemCount`, `DataSource`,
    `LayoutDirection`, `ItemSpacing`, `ItemsPerRow`, `AutoScrollParent`,
    `CloneEvents`, `PreviewItemCount`) to GroupBox defaults; extend the container
    test to assert defaults (and that non-containers lack them).
  - Verify: `cargo test -p cobolt-forms` green.

- [x] **T2 — Renderer: HideBackground / HideCaption / gradient** (R1,R2,R3,R5)
  - Files: `crates/cobolt-forms/src/paint.rs`
  - Do: skip GroupBox frame fill+border when `HideBackground`; skip caption when
    `HideCaption`; when `BackgroundGradientEnabled`, fill via new `grad_dir_mesh`
    (Vertical/Horizontal/DiagonalDown/DiagonalUp/Radial); add mesh unit tests.
  - Verify: `cargo test -p cobolt-forms --features render` green; manual: toggles
    visibly change the GroupBox.

- [x] **T3 — `.cfrm` round-trip test** (R9)
  - Files: `crates/cobolt-forms/src/xml.rs` (test only)
  - Do: save+load a repeating GroupBox with non-default metadata + gradient;
    assert equality.
  - Verify: `cargo test -p cobolt-forms` green.

- [x] **T4 — Properties pane: visual rows + Repeating Group section** (R1,R2,R4,R5,R8,R10)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`, `i18n.rs`
  - Do: add GroupBox visual rows (HideCaption, HideBackground, BackgroundColor,
    gradient enable + start/end colour + direction combo); show a **Repeating
    Group** section (all R8 props) only when `IsRepeatingGroup`. All labels `Tr`.
  - Verify: `cargo build -p cobolt-ide`; `cargo test -p cobolt-ide i18n`.

- [x] **T5 — Context menu Set/Unset Repeating Group** (R7,R8)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`, `i18n.rs`
  - Do: when one GroupBox is selected, add "Set as Repeating Group" (or "Unset
    …") toggling `IsRepeatingGroup` via `Cmd::SetProperty` (undoable); seed
    `ArrayName` with the id when empty on set.
  - Verify: `cargo build -p cobolt-ide`; manual: menu toggles, undo works.

- [x] **T6 — Designer badge + design-time preview clones** (R10,R11)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: draw an array badge on a repeating GroupBox; when `PreviewItemCount > 1`,
    render render-only ghost clones of the group + descendants at layout offsets
    (Vertical/Horizontal/Grid) — never added to the model. Restrict v1 to
    top-level repeating GroupBoxes.
  - Verify: `cargo build -p cobolt-ide`; manual: badge shows; preview lays out;
    model/selection/undo unaffected.

- [x] **T7 — Docs & i18n**
  - Files: `docs/developers-guide-en.md`, `crates/cobolt-ide/src/i18n.rs`
  - Do: document the new GroupBox visual props + repeating groups (config,
    layout, preview, addressing intent); confirm all new `Tr` keys ×6.
  - Verify: `cargo test -p cobolt-ide i18n` green.

- [x] **T8 — Finalize**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: bump patch (fix, per standing directive) + CHANGELOG entry.
  - Verify: `cargo build -p cobolt-ide` + `cargo test -p cobolt-forms` +
    `cargo test -p cobolt-ide i18n` green; manual launch check per plan §6.

## Done criteria
Phase-1/2 acceptance criteria (AC1–AC3, AC5 designer-addressing intent, AC6
template-edit, AC10 layout in preview, AC12 preview, AC13 unchanged default,
AC16 i18n/docs) checked; tests pass; docs updated. Runtime ACs (AC4, AC7–AC9,
AC11, AC15) are Phase 3–5. Do **not** commit/push unless the operator asks.
