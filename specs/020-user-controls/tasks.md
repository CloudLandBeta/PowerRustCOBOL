# Tasks — User Controls + Clipboard + Deletion Confirmation

- **Status:** draft
- **Plan:** ./plan.md   **Date:** 2026-06-29

Ordered, small, independently-verifiable tasks. Each names the files it touches,
the requirement(s) it satisfies, and how to verify it. Check off as completed.

---

## Layer 1: Clipboard & Deletion Confirmation

- [x] **T1 — Deletion confirmation dialog** (R29, R30)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: Modify `delete_selected()` to count controls and event handlers with
    code before deleting. If any handler has code, show a confirmation dialog
    (egui::Window modal) with message "This will remove N controls and M event
    handlers. Continue?" and Cancel/Confirm buttons. On Cancel: do nothing.
    On Confirm: proceed with existing deletion + recycle_control(). Add i18n
    strings: `delete_confirm_title`, `delete_confirm_message`,
    `delete_confirm_cancel`, `delete_confirm_ok` (6 languages).
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → delete a control
    with event handlers → dialog appears. Delete a control without handlers →
    no dialog, deletes immediately. **Covers AC12, AC15, AC18.**

- [x] **T2 — DesignerClipboard struct on CoboltApp** (R28)
  - Files: `crates/cobolt-ide/src/app.rs`
  - Do: Add `DesignerClipboard` struct with `controls: Vec<cobolt_forms::Control>`
    and `source_form: String`. Add a `clipboard: Option<DesignerClipboard>`
    field on `CoboltApp`. This enables cross-form paste.
  - Verify: `cargo check -p cobolt-ide` green.

- [x] **T3 — Copy selected controls (Cmd+C)** (R22, R26)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: Add `copy_selected()` on `DesignerPanel`: clone selected controls +
    all descendants (use `collect_descendants`), relativise positions to
    selection bounding box, strip event handler code (keep binding names),
    store in the app's clipboard. Support multi-select. Wire Cmd+C in the
    designer's input handler. Add i18n strings: `clipboard_copy`.
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → select a Button →
    Cmd+C → no visible change (clipboard populated internally).

- [x] **T4 — Paste from clipboard (Cmd+V)** (R24, R26, R27, R28)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/app.rs`
  - Do: Add `paste_from_clipboard()` on `DesignerPanel`: read from app's
    clipboard, generate new unique IDs for each control (use existing ID
    generation pattern from `Control::new`), offset positions by +20,+20,
    remap parent links (old parent IDs → new IDs), add controls to form,
    select pasted controls. Wire Cmd+V. Support cross-form paste (clipboard
    persists on CoboltApp). Add i18n strings: `clipboard_paste`.
  - Verify: `cargo check -p cobolt-ide` green. Launch IDE → select Button →
    Cmd+C → Cmd+V → new Button at +20,+20 with new ID. **Covers AC13.**

- [x] **T5 — Paste container with children** (R24)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Ensure `paste_from_clipboard()` handles containers: when pasting a
    GroupBox/Panel, all children are pasted with remapped parent IDs. Relative
    positions within the container are preserved.
  - Verify: `cargo check -p cobolt-ide` green. Select GroupBox with 3
    children → Cmd+C → Cmd+V → new GroupBox with 3 children, all new IDs,
    positions preserved. **Covers AC14.**

- [x] **T6 — Cut (Cmd+X)** (R23)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Add `cut_selected()` = `copy_selected()` + `delete_selected()` (which
    now shows confirmation if handlers exist). Wire Cmd+X. Add i18n string:
    `clipboard_cut`.
  - Verify: `cargo check -p cobolt-ide` green. Select Button with handler →
    Cmd+X → confirmation dialog → Confirm → Button removed, clipboard has it.
    **Covers AC15.**

- [x] **T7 — Duplicate (Cmd+D)** (R25)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Add `duplicate_selected()` = `copy_selected()` + `paste_from_clipboard()`
    in one step. Wire Cmd+D. Add i18n string: `clipboard_duplicate`.
  - Verify: `cargo check -p cobolt-ide` green. Select Label → Cmd+D →
    duplicate at +20,+20. **Covers AC16.**

- [x] **T8 — Cross-form paste** (R27, R28)
  - Files: `crates/cobolt-ide/src/app.rs`,
    `crates/cobolt-ide/src/panels/designer.rs`
  - Do: Verify that clipboard on CoboltApp persists across form switches.
    When pasting on a different form, generate new IDs on the target form.
    Event handler code is not transferred.
  - Verify: `cargo check -p cobolt-ide` green. Copy Button on Form A →
    switch to Form B → Cmd+V → Button appears on Form B. **Covers AC17.**

## Layer 2: User Control Definition & Persistence

- [x] **T9 — UserControlDef struct in project model** (R4, R20)
  - Files: `crates/cobolt-ide/src/project_model.rs`
  - Do: Add `UserControlDef { name, width, height, controls: Vec<UserControlEntry> }`
    and `UserControlEntry { id, control_type, x, y, w, h, z_order, properties }`.
    Add `user_controls: Vec<UserControlDef>` to `CoboltProject` (with
    `#[serde(default)]`). Add unit test: `user_control_toml_roundtrip`.
  - Verify: `cargo check -p cobolt-ide` green. `cargo test -p cobolt-ide --
    lib -- user_control_toml` passes.

- [x] **T10 — "Create User Control" context menu** (R1, R2, R3)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/app.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: In the designer's right-click context menu, add "Create User Control"
    (visible only when a single GroupBox is selected). On click: show a name
    prompt dialog (egui::Window modal). Validate: non-empty, valid COBOL
    identifier (letters, digits, hyphens), unique in project. On confirm:
    capture GroupBox + descendants as `UserControlDef` (positions as offsets
    from GroupBox origin), save to project `.toml`. Name is immutable after
    creation. Add i18n strings: `uc_create`, `uc_name_prompt`,
    `uc_name_invalid`, `uc_name_duplicate`.
  - Verify: `cargo check -p cobolt-ide` green. Right-click GroupBox → "Create
    User Control" → enter name → saved to .toml. **Covers AC1, AC10.**

- [x] **T11 — Circular reference detection** (R12)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: When creating a User Control whose children include controls with a
    `UserControl` property, walk the containment graph to check if the new
    name appears in any descendant's `UserControl` reference. Reject with
    error if circular. Add i18n string: `uc_circular_ref`. Add unit test.
  - Verify: `cargo check -p cobolt-ide` green. Attempt to create a UC
    containing itself → error. **Covers AC9.**

- [x] **T12 — Delete User Control definition** (R18)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-ide/src/app.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: Add a way to delete a User Control definition from the project (e.g.
    right-click in Toolbox → "Remove User Control", or via a project settings
    panel). Remove from `CoboltProject.user_controls` and save .toml.
    Deployed instances on forms remain as regular GroupBoxes. Add i18n string:
    `uc_delete_confirm`.
  - Verify: `cargo check -p cobolt-ide` green. Delete UC → removed from
    Toolbox; instances on forms unchanged. **Covers AC8.**

## Layer 3: Toolbox & Deployment

- [x] **T13 — "User Controls" section in Toolbox** (R5)
  - Files: `crates/cobolt-ide/src/panels/toolbox.rs`,
    `crates/cobolt-ide/src/i18n.rs`
  - Do: After the existing categories, add a "User Controls" section that
    reads from the project's `user_controls` list. Each entry is draggable
    (same pattern as built-in controls). Add `dragged_user_control: Option<String>`
    to `ToolboxAction`. Add i18n string: `uc_section_title`.
  - Verify: `cargo check -p cobolt-ide` green. Toolbox shows "User Controls"
    section with defined UCs.

- [x] **T14 — Deploy User Control from Toolbox** (R6, R7, R8, R9, R10)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`,
    `crates/cobolt-forms/src/model.rs`
  - Do: When a User Control is dropped from Toolbox: read `UserControlDef`
    from project, create GroupBox with ID `{Name}-{N}` (N auto-incremented),
    set `UserControl` property to the definition name. For each child: create
    Control with ID `{Name}-{N}-{ChildId}`, parent = new GroupBox,
    position = drop position + child offset, all properties from definition.
    Reuse clipboard paste logic where possible (ID generation, parent remap).
  - Verify: `cargo check -p cobolt-ide` green. Drag UC from Toolbox → controls
    appear with correct IDs and positions. **Covers AC2, AC3.**

- [x] **T15 — Nested User Control deployment** (R11)
  - Files: `crates/cobolt-ide/src/panels/designer.rs`
  - Do: When deploying a UC whose children include controls with a
    `UserControl` property, recursively expand nested UCs: read the nested
    definition, create its children with qualified IDs, and add them to the
    form.
  - Verify: `cargo check -p cobolt-ide` green. Create UC-A containing UC-B →
    deploy UC-A → both levels expanded. **Covers AC4.**

## Layer 4: Properties API & Events

- [x] **T16 — Grouped child properties in properties panel** (R13)
  - Files: `crates/cobolt-ide/src/panels/properties.rs`
  - Do: When a control with a `UserControl` property is selected, show a
    collapsible "Child Controls" section in the properties panel. List each
    child by ID with its properties grouped as `ChildId.PropertyName = value`.
    Reuse existing property row widgets.
  - Verify: `cargo check -p cobolt-ide` green. Select deployed UC instance →
    properties panel shows grouped child properties. **Covers AC5.**

- [x] **T17 — Runtime property API (GetProperty/SetProperty)** (R14, R15)
  - Files: `crates/cobolt-forms/src/render.rs`
  - Do: Handle `INVOKE <uc-id> 'GetProperty' USING '<child>.<prop>'`: find
    child control by qualified ID (`{uc-id}-{child}`), read the property.
    Handle `SetProperty` similarly. Wire through the runtime dispatch.
  - Verify: `cargo check -p cobolt-ide` green. **Covers AC6.**

- [x] **T18 — Qualified event names for child controls** (R16, R17)
  - Files: `crates/cobolt-codegen/src/lib.rs`
  - Do: Verify that the codegen's event loop generates WHEN clauses for
    child controls using their full qualified IDs (e.g. `CUSTOMERCARD-1-BUTTON1`).
    The existing codegen already iterates `ctrl.events` using `ctrl.id` — since
    deployed children have IDs like `CustomerCard-1-Button1`, the handler
    names naturally become `CUSTOMERCARD-1-BUTTON1--ONCLICK`. Verify no changes
    needed; add a test if not.
  - Verify: `cargo check -p cobolt-codegen` green. Generate a form with a
    deployed UC → verify handler names in output. **Covers AC7.**

## Finalization

- [x] **T19 — Docs: developers-guide-en.md User Controls + Clipboard**
  - Files: `docs/developers-guide-en.md`
  - Do: Add a "User Controls" section covering: creation from GroupBox,
    Toolbox, deployment, customisation, property API, event handling, nesting,
    deletion. Add a "Clipboard" section covering Cmd+C/X/V/D shortcuts.
  - Verify: Read the section. Translations untouched.

- [ ] **T20 — Finalize: version bump, full test, manual check**
  - Files: `crates/cobolt-ide/src/version.rs`, `CHANGELOG.md`
  - Do: Bump version. Add CHANGELOG entry. Run full `cargo test`. Launch IDE
    and walk through all manual verification steps from plan §6.
  - Verify: `cargo test` all green. All ACs verified manually.
  - Status: version/changelog updated, full `cargo test` passed, and
    `cargo run -p cobolt-ide` launched the IDE binary. Manual GUI walkthrough
    from plan §6 still needs operator verification.

---

## AC ↔ Task mapping

| AC | Covered by |
|----|------------|
| AC1 | T10 (Create UC from GroupBox) |
| AC2 | T14 (Deploy from Toolbox) |
| AC3 | T14 (Independent instances) |
| AC4 | T15 (Nested UC deployment) |
| AC5 | T16 (Grouped properties panel) |
| AC6 | T17 (Runtime GetProperty) |
| AC7 | T18 (Qualified event names) |
| AC8 | T12 (Delete UC definition) |
| AC9 | T11 (Circular reference detection) |
| AC10 | T10 (Immutable name) |
| AC11 | T1, T3, T4, T6, T7, T10, T12, T13 (i18n strings) |
| AC12 | T1 (Deletion confirmation) |
| AC13 | T4 (Copy + Paste single control) |
| AC14 | T5 (Paste container with children) |
| AC15 | T6 (Cut with confirmation) |
| AC16 | T7 (Duplicate) |
| AC17 | T8 (Cross-form paste) |
| AC18 | T1 (Delete container with handlers) |

## Done criteria

All acceptance criteria in spec.md are checked, tests pass, docs updated, and the
change is split into fix/feature commit(s) per the operator's rules (do **not**
commit/push unless the operator asks).
