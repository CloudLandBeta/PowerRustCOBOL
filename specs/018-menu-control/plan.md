# Plan — MenuControl revamp

- **Status:** draft
- **Spec:** ./spec.md   **Date:** 2026-06-26

## 1. Approach

### Overview

The implementation is split into four layers that build on each other:

1. **Data model** (R1–R6): A `MenuDefinition` struct in `cobolt-forms` with
   serde YAML serialisation + HMAC-SHA256 integrity. The struct holds the full
   tree; each node is a `MenuItem`.
2. **IDE tree editor** (R7–R12, R14): A modal `egui::Window` in the designer
   panel, similar to the existing `EventEditorModal`. Left pane = indented tree
   with action buttons; right pane = detail form for the selected node.
3. **Rendering & interaction** (R13, R15–R25, R28–R30): Canvas rendering reads
   from the loaded `MenuDefinition`; runtime rendering adds pulldown popups via
   `egui::Area`, hover highlighting, sub-menu cascading, accelerator dispatch,
   and event firing through the existing `UiEvent` mechanism.
4. **Programmatic API** (R26–R27): `SetItemEnabled` / `GetItemEnabled` methods
   dispatched via the runtime's `rust_bridge::invoke` pattern, mutating per-item
   enabled state in the live `MenuDefinition`.

### Layer details

**Data model (cobolt-forms)**

- New module `crates/cobolt-forms/src/menu.rs`.
- `MenuDefinition { menu: Vec<MenuItem>, hash: String }`.
- `MenuItem { id, label, item_type, icon, accelerator, action, enabled, items }`.
- `load_menu(path) -> Result<MenuDefinition>`: reads YAML, validates HMAC.
- `save_menu(path, &MenuDefinition)`: serialises YAML, computes & writes HMAC.
- HMAC key: a fixed compile-time constant (resolves Q1). The goal is
  tamper-evidence, not encryption.
- Max depth enforced in `save_menu` (R4).
- New dependencies: `serde_yaml`, `hmac`, `sha2` added to `cobolt-forms/Cargo.toml`
  and the workspace `Cargo.toml`.

**MenuBar model changes (cobolt-forms/model.rs)**

- Remove the `Items` default property from MenuBar (kept for ToolBar/StatusBar).
- Add default properties: `HighlightBgColor` (#4488FF), `HighlightFgColor`
  (#FFFFFF), `SelectedBgColor` (#3366CC), `SelectedFgColor` (#FFFFFF) (R13).
- Update `supported_events` for MenuBar: `["onMenuClick", "onMenuOpen",
  "onMenuClose"]` (R28).

**IDE tree editor (cobolt-ide)**

- New struct `MenuEditorModal` in `crates/cobolt-ide/src/panels/designer.rs`
  (alongside the existing `EventEditorModal`).
- Opened when `InspectorAction::open_menu_editor` is set (triggered by the
  "Edit Menu..." button in properties — R7, R14).
- Left pane: `egui::CollapsingHeader` tree of items with inline icon preview,
  label, accelerator, action summary. Below: toolbar with Add Item, Add
  Sub-menu, Add Separator, Delete, Move Up, Move Down buttons (R8, R9).
- Right pane: detail form with `TextEdit` (label), icon `ComboBox` (catalogue
  dropdown), accelerator `TextEdit` (with format validation), action type
  `ComboBox` (open-form / property / close-application / event), action target
  `TextEdit` (conditional on type), enabled `Checkbox` (R10).
- Save button computes the YAML + HMAC and writes to disk (R11). Cancel
  discards (R12).
- The editor works on a cloned `MenuDefinition` so cancel is free (no undo
  needed).

**Rendering — designer canvas (cobolt-forms/paint.rs)**

- In `draw_control`, the `CT::MenuBar` branch reads the `MenuDefinition` from
  a cached in-memory copy (loaded by the designer on form open / editor save).
  Renders top-level labels horizontally in the glass bar (R15, R16).
- Fallback: no definition → "☰ MenuBar (empty)".

**Rendering — runtime (cobolt-forms/render.rs)**

- MenuBar interactive rendering: each top-level label is a clickable region.
  Clicking opens a dropdown `egui::Area` anchored below the label (R17).
- Dropdown items: icon (16×16 vector path) + label + right-aligned accelerator
  text (R18). Separators: `ui.separator()` (R19).
- Hover: `HighlightBgColor`/`HighlightFgColor` applied on the hovered row (R20).
- Sub-menus: items with `items` open a cascading `egui::Area` to the right on
  hover (R21). Depth capped at 3 by the data model.
- Click: close all open areas, execute action, push `UiEvent` with
  `value = Some(item_id)` for `onMenuClick` (R22).
- Disabled: dimmed alpha, no click/hover response (R23).
- Accelerators: collected into a `HashMap<Modifiers+Key, item_id>` on menu
  load. Each frame, `ui.input()` is checked for matching key combos; if found
  and item is enabled, the action fires (R24).
- Click-outside: `ui.input().pointer.any_pressed()` outside all open areas
  closes them (R25).

**Icons (cobolt-forms/paint.rs)**

- New module `crates/cobolt-forms/src/icons.rs` with `pub fn
  draw_menu_icon(painter, rect, icon_name, color)` and the full catalogue
  constant `MENU_ICON_NAMES` (R29, R30). Follows the same hand-drawn pattern
  as `nv_icon_clock`, `nv_icon_robot`, etc.
- 100+ icons across 14 categories (document, edit, navigation, action,
  UI/view, communication, social, people, media, data, system, status,
  commerce, file/folder). Each is 10–20 lines of painter calls.

**Programmatic API (runtime)**

- `SetItemEnabled(item_id, bool)` and `GetItemEnabled(item_id) -> bool`
  dispatched through the existing `rust_bridge::invoke` mechanism (R26, R27).
- The live `MenuDefinition` is stored in the form runtime state alongside the
  control; `invoke` mutates/queries the `enabled` field of the matching
  `MenuItem`.

**i18n**

- New `Tr` fields for: tree editor window title, button labels (Add Item, Add
  Sub-menu, Add Separator, Delete, Move Up, Move Down, Save, Cancel), detail
  panel labels (Label, Icon, Accelerator, Action, Target, Enabled), property
  labels (HighlightBgColor, etc.), error messages (hash mismatch, depth
  exceeded). All 6 languages (R14 compliance, AC11).

## 2. Affected crates / files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `serde_yaml`, `hmac`, `sha2` to `[workspace.dependencies]` |
| `crates/cobolt-forms/Cargo.toml` | Add `serde_yaml`, `hmac`, `sha2` deps |
| `crates/cobolt-forms/src/lib.rs` | `pub mod menu;` |
| `crates/cobolt-forms/src/menu.rs` | **New** — `MenuDefinition`, `MenuItem`, load/save/HMAC |
| `crates/cobolt-forms/src/model.rs` | MenuBar default props (remove `Items`, add colour props, update `supported_events`) |
| `crates/cobolt-forms/src/paint.rs` | `draw_menu_icon()` (20 icons), update `CT::MenuBar` canvas rendering |
| `crates/cobolt-forms/src/render.rs` | MenuBar interactive runtime: pulldown popups, hover, sub-menus, accelerators, events |
| `crates/cobolt-forms/src/xml.rs` | `seed_missing_props`: seed new MenuBar props on existing controls |
| `crates/cobolt-ide/src/panels/designer.rs` | `MenuEditorModal` struct + `show_menu_editor()`, load/cache menu on form open |
| `crates/cobolt-ide/src/panels/properties.rs` | MenuBar section: "Edit Menu..." button + colour properties |
| `crates/cobolt-ide/src/i18n.rs` | ~25 new `Tr` fields × 6 languages |
| `docs/developers-guide-en.md` | MenuBar section: tree editor, YAML format, actions, accelerators, icons, COBOL API |

## 3. Data / model changes

### New on-disk format: `<control-id>.menu.yaml`

```yaml
menu:
  - id: "file"
    label: "File"
    type: action
    items:
      - id: "file-new"
        label: "New"
        type: action
        icon: "doc-new"
        accelerator: "Cmd+N"
        action: "open-form:NewProject"
        enabled: true
      - id: "sep-1"
        type: separator
      - id: "file-exit"
        label: "Exit"
        type: action
        icon: "x-circle"
        action: "close-application"
        enabled: true
  - id: "edit"
    label: "Edit"
    type: action
    items:
      - id: "edit-undo"
        label: "Undo"
        type: action
        icon: "arrow-left"
        accelerator: "Cmd+Z"
        action: "event"
        enabled: true
hash: "a1b2c3d4e5f6..."
```

### MenuBar control property changes

| Property | Old | New |
|----------|-----|-----|
| `Items` | `""` (flat text) | **Removed** from MenuBar (kept for ToolBar/StatusBar) |
| `HighlightBgColor` | — | `"#4488FF"` |
| `HighlightFgColor` | — | `"#FFFFFF"` |
| `SelectedBgColor` | — | `"#3366CC"` |
| `SelectedFgColor` | — | `"#FFFFFF"` |

### MenuBar `supported_events` change

Old: `&["onClick"]` (default fallthrough).
New: `&["onMenuClick", "onMenuOpen", "onMenuClose"]`.

### Migration

- `seed_missing_props` in `xml.rs` seeds the four colour properties on existing
  MenuBar controls at load time.
- Existing MenuBar controls that had flat `Items` text: the text is ignored
  (the new system reads from YAML). Users must recreate their menus in the tree
  editor. Since the old Items property was non-functional (no actions, no
  hierarchy), this is acceptable.

## 4. Key decisions & alternatives

**D1: YAML vs embedding menu in .cfrm XML**
- Decision: Separate YAML file.
- Why: The user explicitly requested YAML with integrity hash. Keeps the .cfrm
  format unchanged. YAML is human-readable for debugging.
- Rejected: Embedding in .cfrm XML — would complicate the XML parser and lose
  the integrity-hash requirement.

**D2: HMAC key — fixed constant vs project-derived**
- Decision: Fixed compile-time constant (resolves Q1).
- Why: The goal is tamper-evidence ("only the IDE wrote this"), not encryption.
  A project-derived key adds complexity for no security gain.
- Rejected: Per-project key — would require key storage/management.

**D3: Event parameter — item `id` only (resolves Q2)**
- Decision: `onMenuClick` fires with `value = Some(item_id)`.
- Why: The `id` is stable (developer-chosen or auto-generated); labels can
  change. The developer's event handler uses `EVALUATE WS-MENU-ITEM-ID` to
  branch on the clicked item.
- Rejected: Passing both id and label — adds complexity, the handler rarely
  needs the label.

**D4: Platform-native accelerator symbols (resolves Q3)**
- Decision: Render `⌘` on macOS, `Ctrl` on Windows/Linux.
- Why: Matches user expectation. Detected via `cfg!(target_os = "macos")` at
  compile time.
- Rejected: Always showing `Cmd` or always `Ctrl` — confuses one platform.

**D5: Dropdown rendering — `egui::Area` vs custom popup**
- Decision: Use `egui::Area` with `Order::Foreground` for dropdowns.
- Why: Reuses the same pattern as `glass_combo_popup` already in the codebase.
  Areas float above all other content, handle click-outside naturally.
- Rejected: Custom overlay painting — would need manual z-ordering and input
  handling.

## 5. Risks & mitigations

- **Risk:** Adding 3 new crate dependencies (`serde_yaml`, `hmac`, `sha2`)
  increases compile time.
  → **Mitigation:** These are small, well-maintained crates. `serde` is already
  a dependency. The compile-time impact is ~2–3 seconds.

- **Risk:** Sub-menu cascading with `egui::Area` may have z-order issues when
  multiple levels are open simultaneously.
  → **Mitigation:** Each sub-menu uses a distinct `egui::Id` and
  `Order::Foreground`. Test with 3 levels open at once.

- **Risk:** Accelerator key combos may conflict with OS shortcuts (Cmd+Q,
  Cmd+W, etc.) or IDE shortcuts.
  → **Mitigation:** Accelerators only fire when the form is running (preview
  or compiled), not in the designer. Document this in the guide.

- **Risk:** 100+ hand-drawn vector icons is a significant amount of painter code.
  → **Mitigation:** Each icon is 10–20 lines (simple strokes/arcs). Factored
  into a dedicated `icons.rs` module (~1500–2000 lines). Organised by category
  with a flat match statement for dispatch.

## 6. Test strategy

### Unit tests (cobolt-forms)

- `menu::tests::round_trip_yaml` — create a `MenuDefinition`, save to YAML,
  reload, assert equality. Reports: field-by-field comparison.
- `menu::tests::hmac_validates` — save, reload (passes), tamper with one byte,
  reload (fails with hash error). Reports: pass/fail + error message.
- `menu::tests::depth_limit` — attempt to save a 4-level tree, assert error.
- `menu::tests::accelerator_parse` — parse "Cmd+N", "Shift+Ctrl+S", etc.,
  assert correct `Modifiers` + `Key`. Reports: each combo pass/fail.

### Integration tests (cobolt-ide — manual/visual)

1. **Tree editor flow:** Drop a MenuBar → properties shows "Edit Menu..." →
   click → modal opens → add "File" with sub-items "New" (icon: doc-new,
   accel: Cmd+N), separator, "Exit" (action: close-application) → Save → bar
   shows "File". **Verify:** YAML file created alongside .cfrm.
2. **Runtime dropdown:** Run the form → click "File" → dropdown opens with
   icons, labels, accelerator text → hover highlights → click "Exit" → app
   closes. **Verify:** hover colours match properties.
3. **Sub-menu cascade:** Add a 2nd-level item with children → runtime shows
   sub-menu on hover to the right. 3rd level works. 4th level prevented in
   editor.
4. **Accelerator:** Press Cmd+N at runtime → "New" action fires without
   opening the menu.
5. **Programmatic disable:** In COBOL handler, `INVOKE MENU1 'SetItemEnabled'
   USING 'file-new' WS-FALSE` → "New" item appears dimmed, click ignored.
6. **Tamper detection:** Edit the YAML file externally → run → error shown,
   menu not rendered.
7. **ToolBar/StatusBar unchanged:** Existing ToolBar with flat items still
   renders correctly.

## 7. Steering compliance

- [x] i18n: ~25 new `Tr` fields planned for all 6 languages
- [x] Generated-code banner + regenerate-on-action contract preserved (YAML is
      a design artifact, not generated code)
- [x] English dev guide updated (MenuBar section); translations untouched
- [x] Fix vs feature: **fix** (pre-prod override) → bump `z` in version.rs,
      CHANGELOG under Fix, announce on f=97
- [x] No "cobolt" in user-facing text; COBOL identifiers English
