# Handoff — User Controls + Clipboard + Deletion Confirmation

## ⚠️ CRITICAL RULE — NO REGRESSIONS, NO SIDE CHANGES

**Every change you make MUST NOT break any existing behavior.** This is
non-negotiable. Specifically:

- **Do NOT modify any rendering code** (paint.rs glass rendering, notch masks,
  container clipping, image rounding, arc computations). These systems are
  fragile and recently stabilised.
- **Do NOT modify the egui version** or any dependency versions.
- **Do NOT modify any existing control type's behavior** — Button, TextBox,
  GroupBox, Panel, PictureBox, etc. must work exactly as before.
- **Do NOT modify the event dispatch system** (control_pointer_events,
  UiEvent, FormEvent) unless adding NEW event types.
- **Do NOT modify the `.cfrm` XML format** for existing controls — only ADD
  new properties/attributes for User Control instances.
- **Do NOT modify the codegen** (cobolt-codegen) unless strictly needed for
  qualified event names, and even then, only ADD — never change existing
  generation.
- **Do NOT modify i18n strings** that already exist — only ADD new ones.

**If you are unsure whether a change could affect existing behavior, STOP and
ask.** It is better to leave a feature partially implemented than to break
something that works.

**Test after EVERY change:** `cargo check -p cobolt-ide` must pass. Run
`cargo test -p cobolt-forms --lib` to verify no model/XML regressions.

---

## What to implement

Read these files in order:
1. `specs/020-user-controls/spec.md` — the requirements (R1–R30, AC1–AC18)
2. `specs/020-user-controls/plan.md` — the design (4 layers, key decisions)
3. This file — constraints and implementation guidance

## Implementation order

Implement in this EXACT order. Each layer must compile and pass tests before
starting the next.

### Layer 1: Clipboard & deletion confirmation (R22–R30)

**Files to modify:**
- `crates/cobolt-ide/src/panels/designer.rs` — clipboard operations, deletion dialog
- `crates/cobolt-ide/src/app.rs` — clipboard field on `CoboltApp`, keyboard shortcuts
- `crates/cobolt-ide/src/i18n.rs` — new strings for clipboard and deletion dialog

**What to do:**

1. Add a `DesignerClipboard` struct:
   ```rust
   struct DesignerClipboard {
       controls: Vec<cobolt_forms::Control>,
       source_form: String,
   }
   ```
   Store it on `CoboltApp` (not `DesignerPanel`) for cross-form paste.

2. Implement `copy_selected()` on `DesignerPanel`:
   - Clone selected controls + all descendants (use `collect_descendants`)
   - Relativise positions to the selection bounding box
   - Strip event handler CODE (keep bindings/names only)
   - Store in the app's clipboard

3. Implement `paste_from_clipboard()` on `DesignerPanel`:
   - Read from the app's clipboard
   - Generate new unique IDs for each control (use existing ID generation pattern)
   - Offset positions by +20, +20 from original
   - Fix parent links (remap old parent IDs to new IDs)
   - Add controls to the form
   - Select the pasted controls

4. Implement `cut_selected()` = copy + delete (with confirmation if handlers exist)

5. Implement `duplicate_selected()` = copy + paste in one step

6. Modify `delete_selected()`:
   - Before deleting, count controls with event handlers that have code
   - If count > 0, show a confirmation dialog (egui::Window modal)
   - Dialog shows: "This will remove N controls and M event handlers. Continue?"
   - Cancel: do nothing. Confirm: proceed with existing deletion logic
   - The existing `recycle_control()` already saves deleted code — USE IT

7. Wire keyboard shortcuts in the designer's input handler:
   - Cmd+C → copy_selected
   - Cmd+X → cut_selected
   - Cmd+V → paste_from_clipboard
   - Cmd+D → duplicate_selected

8. Add i18n strings (6 languages):
   - Confirmation dialog title, message template, Cancel/Confirm buttons
   - Clipboard status messages (optional)

**What NOT to do:**
- Do NOT change how controls are rendered
- Do NOT change the existing `delete_selected()` logic — only ADD the
  confirmation dialog before it
- Do NOT change how IDs are generated for new controls — reuse the existing
  pattern

### Layer 2: User Control definition & persistence (R1–R4, R20)

**Files to modify:**
- `crates/cobolt-ide/src/project_model.rs` — new structs
- `crates/cobolt-ide/src/panels/designer.rs` — right-click menu, name dialog
- `crates/cobolt-ide/src/app.rs` — save/load project with user controls

**What to do:**

1. Add to `project_model.rs`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct UserControlDef {
       pub name: String,
       pub width: i32,
       pub height: i32,
       pub controls: Vec<UserControlEntry>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct UserControlEntry {
       pub id: String,
       pub control_type: String,
       pub x: i32,
       pub y: i32,
       pub w: i32,
       pub h: i32,
       pub z_order: i32,
       pub properties: std::collections::HashMap<String, String>,
   }
   ```

2. Add to `CoboltProject`:
   ```rust
   #[serde(default)]
   pub user_controls: Vec<UserControlDef>,
   ```

3. In the designer's right-click context menu, add "Create User Control"
   (only when a GroupBox is selected):
   - Show a name prompt dialog (egui::Window modal)
   - Validate: non-empty, valid COBOL identifier, unique in project
   - Capture the GroupBox + descendants as a `UserControlDef`
   - Positions stored as offsets from GroupBox origin
   - Save to project `.toml`

4. Add circular reference detection:
   - When creating a UC that contains children with `UserControl` property,
     walk the containment graph to check if the new name appears
   - Reject with error message if circular

**What NOT to do:**
- Do NOT modify the Form model or `.cfrm` format in this layer
- Do NOT change how GroupBox works — you're only READING its data

### Layer 3: Toolbox & deployment (R5–R10)

**Files to modify:**
- `crates/cobolt-ide/src/panels/toolbox.rs` — "User Controls" section
- `crates/cobolt-ide/src/panels/designer.rs` — drop handler for UC deployment

**What to do:**

1. In `toolbox.rs`, add a "User Controls" section after the existing categories:
   - Read from the project's `user_controls` list
   - Each entry is draggable (same pattern as built-in controls)
   - Use a special marker to distinguish UC drags from built-in control drags
     (e.g. `ToolboxAction::dragged_user_control: Option<String>`)

2. In the designer's drop handler, when a User Control is dropped:
   - Read the `UserControlDef` from the project
   - Create a GroupBox with ID `{Name}-{N}` where N is auto-incremented
   - Set `UserControl` property to the definition name
   - For each child in the definition, create a Control with:
     - ID = `{Name}-{N}-{ChildId}`
     - parent = the new GroupBox ID
     - position = drop position + child offset
     - All properties from the definition
   - For nested User Controls (children with type "GroupBox" and a
     `UserControl` property), recursively expand
   - Add all controls to the form

3. Reuse the clipboard paste logic where possible (ID generation, parent
   remapping, position offsetting).

**What NOT to do:**
- Do NOT modify the existing Toolbox categories or built-in control entries
- Do NOT modify how built-in controls are dropped onto the canvas

### Layer 4: Properties API & events (R13–R17)

**Files to modify:**
- `crates/cobolt-ide/src/panels/properties.rs` — grouped child property view
- `crates/cobolt-forms/src/render.rs` — runtime property API (optional, can defer)

**What to do:**

1. In the properties panel, when a control with a `UserControl` property is
   selected:
   - Show a collapsible "Child Controls" section
   - List each child control with its ID
   - Under each child, show its properties (reuse existing property rows)
   - Format: `ChildId.PropertyName = value`

2. For the runtime property API (can be deferred to a follow-up):
   - Handle `INVOKE <uc-id> 'GetProperty' USING '<child>.<prop>'`
   - Find the child control by qualified ID
   - Read/write the property

**What NOT to do:**
- Do NOT change how properties are displayed for regular (non-UC) controls
- Do NOT change the event dispatch system — qualified IDs already work

## Key patterns to reuse

| Pattern | Where it exists | How to reuse |
|---------|----------------|--------------|
| Control deep-copy | `collect_descendants()` in containers.rs | Copy all children of a container |
| ID generation | `Control::new()` in model.rs | Generate unique sequential IDs |
| Recycle bin | `Form::recycle_control()` in model.rs | Preserve deleted handler code |
| Context menu | `resp.context_menu()` in designer.rs line ~2576 | Add "Create User Control" item |
| Drag from toolbox | `ToolboxAction::dragged_type` in toolbox.rs | Add `dragged_user_control` |
| Modal dialog | `EventEditorModal` / `MenuEditorModal` in designer.rs | Name prompt, confirmation dialogs |
| Project persistence | `CoboltProject` serde in project_model.rs | Add `user_controls` field |

## Files you MUST NOT modify

- `crates/cobolt-forms/src/paint.rs` — rendering, glass, clipping, notch masks
- `crates/cobolt-forms/src/icons.rs` — icon catalogue
- `crates/cobolt-forms/src/menu.rs` — menu data model
- `crates/cobolt-forms/src/fonts.rs` — font loading
- `crates/cobolt-forms/src/theme.rs` — theme catalogue
- `crates/cobolt-forms/src/theme_pack.rs` — asset pack loading
- `crates/cobolt-media/` — media/animation
- `crates/cobolt-runtime/` — COBOL interpreter (unless wiring property API)
- `crates/cobolt-ide/src/main.rs` — app entry point
- `crates/cobolt-ide/src/panels/editor.rs` — COBOL code editor
- `crates/cobolt-ide/src/panels/doc_viewer.rs` — documentation viewer

## Verification checklist

After EACH layer, verify:

- [ ] `cargo check -p cobolt-ide` — 0 errors
- [ ] `cargo test -p cobolt-forms --lib` — all tests pass (currently 44)
- [ ] Launch IDE → open existing form → all controls render correctly
- [ ] Launch IDE → all existing toolbox categories present and functional
- [ ] Launch IDE → right-click context menu on controls still works
- [ ] Launch IDE → delete a control → still works (with new confirmation
      dialog if handlers exist)
- [ ] No new warnings introduced (check `cargo check` output)

## i18n strings to add

Add these to `Tr` in `i18n.rs` with all 6 translations:

```
// Clipboard
clipboard_cut, clipboard_copy, clipboard_paste, clipboard_duplicate

// Deletion confirmation
delete_confirm_title, delete_confirm_message, delete_confirm_cancel,
delete_confirm_ok

// User Controls
uc_section_title, uc_create, uc_name_prompt, uc_name_invalid,
uc_name_duplicate, uc_circular_ref, uc_delete_confirm
```

## Task list

The full ordered task list is in `specs/020-user-controls/tasks.md` — **20 tasks**
across 4 layers. Implement them IN ORDER. Each task names the files to modify,
the requirements it satisfies, and how to verify it.

### Summary

| Layer | Tasks | What |
|-------|-------|------|
| 1. Clipboard & Deletion | T1–T8 | Deletion confirmation dialog, Copy (Cmd+C), Paste (Cmd+V), paste containers, Cut (Cmd+X), Duplicate (Cmd+D), cross-form paste |
| 2. UC Definition | T9–T12 | UserControlDef struct, .toml persistence, "Create User Control" context menu + name dialog, circular reference detection, delete UC definition |
| 3. Toolbox & Deploy | T13–T15 | "User Controls" Toolbox section, deploy from Toolbox (deep copy + re-ID), nested UC expansion |
| 4. Properties & Events | T16–T18 | Grouped child properties in panel, runtime GetProperty/SetProperty, qualified event handler names |
| Finalize | T19–T20 | Docs, version bump, manual checks |

### AC ↔ Task mapping

| AC | Task |
|----|------|
| AC1 (Create UC) | T10 |
| AC2 (Deploy) | T14 |
| AC3 (Independent) | T14 |
| AC4 (Nesting) | T15 |
| AC5 (Properties panel) | T16 |
| AC6 (Runtime GetProperty) | T17 |
| AC7 (Qualified events) | T18 |
| AC8 (Delete UC def) | T12 |
| AC9 (Circular ref) | T11 |
| AC10 (Immutable name) | T10 |
| AC11 (i18n) | T1, T3, T4, T6, T7, T10, T12, T13 |
| AC12 (Delete instance confirm) | T1 |
| AC13 (Copy+Paste single) | T4 |
| AC14 (Copy+Paste container) | T5 |
| AC15 (Cut with confirm) | T6 |
| AC16 (Duplicate) | T7 |
| AC17 (Cross-form paste) | T8 |
| AC18 (Delete container confirm) | T1 |

## Related spec: 021-control-events

Spec 021 (comprehensive fireable events) has its own task list at
`specs/021-control-events/tasks.md` — 16 tasks. It is independent from this
spec and can be implemented in parallel, but the same NO REGRESSIONS rule
applies. If implementing both, do spec 020 first (clipboard/UC), then 021
(events), to avoid merge conflicts in `model.rs` and `render.rs`.

## Open questions (resolved)

- **Q1 (event code):** Store event bindings only, NOT code. ✓
- **Q2 (.toml vs files):** Store in `.toml`. ✓
- **Q3 (thumbnails):** Name + icon only for v1. ✓
