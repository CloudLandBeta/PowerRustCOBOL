# Plan — User Controls + Clipboard + Deletion Confirmation

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-29

## 1. Approach

The implementation is split into four layers, ordered so each builds on the
previous:

### Layer 1: Clipboard & deletion confirmation (R22–R30)

These are general-purpose features needed by ALL controls. Implementing them
first creates the foundation that User Control deployment reuses.

**Clipboard** — New `DesignerClipboard` struct on `DesignerPanel` holding a
`Vec<Control>` (the cloned tree). Copy captures the selected controls +
descendants with positions relativised to the selection bounding box.
Paste creates new controls with auto-generated IDs at +20,+20 offset.
Cut = copy + delete (with confirmation). Duplicate = copy + paste.
The clipboard persists as a field on `CoboltApp` (not per-designer) for
cross-form paste (R27, R28). Keyboard shortcuts wired in the designer's
input handler.

**Deletion confirmation** — Modify `delete_selected()` in `designer.rs` to
count event handlers with code before deleting. If count > 0, show a
confirmation dialog (egui `Window` modal). The existing `recycle_control()`
already preserves deleted code (R30).

### Layer 2: User Control definition & persistence (R1–R4, R20)

**Model** — New `UserControlDef` struct in `project_model.rs`:
`{ name, width, height, controls: Vec<UserControlEntry> }` where
`UserControlEntry` has `{ id, control_type, x, y, w, h, z_order, properties }`.
Stored in `CoboltProject` under a new `user_controls: Vec<UserControlDef>`
field, serialised to the `[user-controls.<name>]` section in `.toml`.

**Creation** — In the designer's right-click context menu, add "Create User
Control" (only visible when a GroupBox is selected). Shows a name prompt
dialog. On confirm, captures the GroupBox + descendants as a
`UserControlDef`, writes to the project `.toml`, and refreshes the Toolbox.

**Circular reference detection** (R12) — Before saving a definition that
contains a child with a `UserControl` property, walk the containment graph
and reject if the new name appears in any descendant's `UserControl`
reference.

### Layer 3: Toolbox & deployment (R5–R10)

**Toolbox** — Add a "User Controls" section to `ToolboxPanel` that reads
from `app.cobolt_project.user_controls`. Each entry is draggable like the
built-in control entries. The Toolbox already has a filter; User Controls
are searchable by name.

**Deployment** — When a User Control is dropped from the Toolbox, the
designer:
1. Reads the `UserControlDef` from the project.
2. Creates a new GroupBox with ID `{Name}-{seq}` and a `UserControl`
   property set to the definition name.
3. For each child in the definition, creates a `Control` with ID
   `{Name}-{seq}-{ChildId}`, parent = the new GroupBox, and position =
   GroupBox origin + child offset.
4. Recursively expands nested User Controls (R11).
5. Adds all controls to the form.

This reuses the same deep-copy logic as the clipboard paste (Layer 1).

### Layer 4: Properties API & events (R13–R17)

**Properties panel** — When a deployed User Control instance is selected,
the properties panel shows a collapsible "Child Controls" section listing
each child with its properties grouped by `ChildId.Section`.

**Runtime property API** — `GetProperty('ChildId.PropName')` and
`SetProperty('ChildId.PropName', value)` are dispatched by finding the
child control by its qualified ID and reading/writing the property. Wired
through the existing `rust_bridge::invoke` mechanism.

**Events** — Child controls within a deployed instance already have unique
IDs (`USERCTRL1--BUTTON1`). The codegen's `write_event_loop` already
generates CALL statements for each control's events using the control ID.
The qualified naming `USERCTRL1--BUTTON1--ONCLICK` falls out naturally
from the existing ID scheme — no special codegen needed.

## 2. Affected crates / files

| File | Change |
|------|--------|
| `crates/cobolt-ide/src/project_model.rs` | `UserControlDef`, `UserControlEntry` structs; `user_controls` field on `CoboltProject` |
| `crates/cobolt-ide/src/panels/designer.rs` | Clipboard (copy/cut/paste/duplicate), deletion confirmation dialog, "Create User Control" context menu, User Control deployment |
| `crates/cobolt-ide/src/panels/toolbox.rs` | "User Controls" section reading from project model |
| `crates/cobolt-ide/src/panels/properties.rs` | Grouped child-property view for deployed instances |
| `crates/cobolt-ide/src/app.rs` | Clipboard field on `CoboltApp` (cross-form), keyboard shortcut wiring, project save/load |
| `crates/cobolt-ide/src/i18n.rs` | ~15 new `Tr` fields × 6 languages |
| `crates/cobolt-forms/src/model.rs` | `UserControl` property on deployed GroupBox instances |
| `crates/cobolt-forms/src/render.rs` | Runtime property API dispatch (GetProperty/SetProperty) |
| `crates/cobolt-codegen/src/lib.rs` | No changes needed — qualified IDs already work |
| `docs/developers-guide-en.md` | User Controls section, Clipboard shortcuts |

## 3. Data / model changes

### Project `.toml` — new `[user-controls]` section

```toml
[user-controls.CustomerCard]
width = 300
height = 200

[[user-controls.CustomerCard.controls]]
id = "Label1"
type = "Label"
x = 10
y = 10
w = 80
h = 20
z_order = 0
properties = { Caption = "Name:" }

[[user-controls.CustomerCard.controls]]
id = "Button1"
type = "Button"
x = 200
y = 160
w = 80
h = 28
z_order = 1
properties = { Caption = "Save" }
```

### `.cfrm` — deployed instance

Deployed instances are saved as regular GroupBox controls with an extra
`UserControl` property:

```xml
<Control id="CustomerCard-1" type="GroupBox" x="50" y="100" w="300" h="200"
         parent="">
  <Property name="UserControl">CustomerCard</Property>
</Control>
<Control id="CustomerCard-1-Label1" type="Label" x="60" y="110" w="80" h="20"
         parent="CustomerCard-1">
  <Property name="Caption">Name:</Property>
</Control>
```

### Designer clipboard

```rust
struct DesignerClipboard {
    controls: Vec<Control>,  // cloned tree, positions relative to bounding box
    source_form: String,     // form name where copy happened
}
```

Stored on `CoboltApp` (not `DesignerPanel`) for cross-form paste.

## 4. Key decisions & alternatives

**D1: Clipboard storage — CoboltApp vs OS clipboard**
- Decision: Internal `DesignerClipboard` on `CoboltApp`.
- Why: Controls are complex Rust structs, not serialisable to the OS clipboard
  format. Internal clipboard is simpler and supports cross-form paste.
- Rejected: OS clipboard with XML/JSON serialisation — adds complexity,
  no inter-app paste use case.

**D2: User Control in .toml vs separate .ucfrm files**
- Decision: Store in `.toml` (resolves Q2).
- Why: User Controls are typically small. The `.toml` format handles nested
  tables well. No file management overhead.
- Rejected: Separate files — adds file discovery, loading, error handling.

**D3: Event handler code in User Control definition**
- Decision: Store event bindings only, not code (resolves Q1).
- Why: Each instance diverges; pre-populated code would confuse. The developer
  writes per-instance handlers.
- Rejected: Storing code — instances would share code that quickly diverges.

**D4: Deployment reuses clipboard logic**
- Decision: User Control deployment internally creates a clipboard-like
  snapshot and pastes it, reusing the same deep-copy + re-ID logic.
- Why: DRY — one code path for both "paste controls" and "deploy User Control".
- Rejected: Separate deployment logic — duplicate code, divergent bugs.

**D5: Child IDs use dash separator**
- Decision: `UserCtrl1-Button1` (dash, not dot).
- Why: COBOL identifiers use dashes. Dots are member-access operators.
  The codegen already handles `ID--EVENT` naming.
- Rejected: Dot separator — conflicts with COBOL member-access syntax.

## 5. Risks & mitigations

- **Risk:** Large clipboard (GroupBox with 50+ controls) may be slow to paste.
  → **Mitigation:** Controls are lightweight structs; 50 copies is microseconds.

- **Risk:** Circular reference detection for nested User Controls.
  → **Mitigation:** Simple DFS walk on the containment graph at creation time.
  User Controls are project-scoped, so the graph is small.

- **Risk:** Qualified event names (`USERCTRL1--BUTTON1--ONCLICK`) may exceed
  COBOL's 30-character identifier limit.
  → **Mitigation:** Truncate/hash long names in codegen, same as existing
  controls with long IDs. Document the limit.

- **Risk:** Deletion confirmation dialog interrupts workflow.
  → **Mitigation:** Only shown when event handlers with code exist. Simple
  "Delete" with no handlers proceeds immediately.

## 6. Test strategy

### Unit tests (cobolt-forms)

- `model::tests::user_control_property` — deploy a UserControl GroupBox,
  verify the `UserControl` property is set.

### Unit tests (cobolt-ide)

- `project_model::tests::user_control_toml_roundtrip` — create a
  `UserControlDef`, serialise to TOML, deserialise, assert equality.
- `project_model::tests::circular_reference_detected` — attempt to create
  a User Control containing itself, assert rejection.

### Manual / visual checks

1. **Create**: Right-click GroupBox with children → "Create User Control" →
   name it → appears in Toolbox.
2. **Deploy**: Drag from Toolbox → controls appear with new IDs → edit one
   instance → other instances unaffected.
3. **Nesting**: Create UC-A containing UC-B → deploy UC-A → both levels
   expanded correctly.
4. **Copy/Paste**: Select Button → Cmd+C → Cmd+V → new Button at offset.
5. **Copy container**: Select GroupBox with children → Cmd+C → Cmd+V →
   all children copied with new IDs.
6. **Cut with handlers**: Select control with code → Cmd+X → confirmation
   dialog → Cancel keeps it; Confirm removes + recycles.
7. **Duplicate**: Cmd+D → clone at offset.
8. **Cross-form**: Copy on Form A → switch to Form B → Cmd+V → works.
9. **Delete confirmation**: Delete Panel with event-handler children →
   confirmation dialog with counts.
10. **Toolbox removal**: Delete User Control definition → removed from
    Toolbox; instances remain on forms.

## 7. Steering compliance

- [ ] i18n: ~15 new `Tr` fields in 6 languages (Create User Control, name
      prompt, confirmation dialogs, Toolbox section, clipboard status)
- [ ] Generated-code banner + regenerate-on-action contract preserved
      (deployed instances are regular controls; codegen unchanged)
- [ ] English dev guide updated (User Controls + Clipboard); translations
      untouched
- [ ] Fix vs feature: **feature** — confirm with operator whether pre-prod
      override applies (z bump) or real feature (y bump)
- [ ] No "cobolt" in user-facing text; COBOL identifiers English
