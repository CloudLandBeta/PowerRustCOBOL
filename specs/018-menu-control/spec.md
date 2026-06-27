# Spec — MenuControl revamp

- **Status:** draft
- **Folder:** specs/018-menu-control/
- **Author:** Claude (spec-driven)   **Date:** 2026-06-26

## 1. Overview

Replace the current flat text-list MenuBar with a full **pulldown menu system**
supporting 3-level hierarchical menus, per-item actions, accelerator keys,
vector icons, separators, and programmatic enable/disable. The menu structure
is authored in a **modal tree editor** in the IDE and persisted as a
YAML file with HMAC-SHA256 integrity so only the IDE can modify it. At runtime
the menu bar renders interactive pulldown menus that fire events or execute
built-in actions (open form, set property, close application, call COBOL
paragraph).

ToolBar and StatusBar retain their current flat-items behaviour; only MenuBar
is revamped.

## 2. Goals / Non-goals

### Goals

- A usable, discoverable menu system a COBOL developer can author without
  writing YAML by hand — the tree editor is the primary authoring surface.
- 3-level pulldown menus (top bar → dropdown → submenu) matching standard
  desktop application conventions.
- Built-in action types covering the most common menu operations; custom COBOL
  paragraphs for everything else.
- Per-item accelerator keys rendered on the menu and wired to the runtime.
- Per-item hand-drawn vector icons (a fixed built-in icon catalogue).
- Visual customisation: selected/highlighted item colours via control properties.
- Integrity: the YAML file is tamper-evident via HMAC-SHA256.
- Programmatic enable/disable of individual items at runtime from COBOL.

### Non-goals

- Context menus (right-click) — future spec.
- Bitmap/raster icons — only the built-in vector icon set.
- ToolBar and StatusBar redesign — unchanged in this spec.
- Drag-and-drop reordering in the tree editor (buttons suffice for v1).
- Theming the menu beyond selected/highlighted colours (inherits the glass
  style from the form).
- Menu bar docking/floating — the MenuBar is a positioned control like any
  other; full-width docking is achieved by the user setting Dock = Top.

## 3. User stories

- As a **form designer**, I want to visually build a menu hierarchy in a tree
  editor, so that I can define menus without editing YAML by hand.
- As a **form designer**, I want to assign accelerator keys and icons to menu
  items, so that my application looks professional and is keyboard-accessible.
- As a **form designer**, I want to assign actions to menu items (open a form,
  set a property, close the app, or call a COBOL paragraph), so that common
  operations need no code.
- As a **COBOL developer**, I want to enable/disable menu items at runtime from
  my COBOL code, so that the menu reflects application state.
- As a **form designer**, I want to set the highlight and selection colours of
  menu items via control properties, so that the menu matches my application
  style.
- As an **end user**, I want pulldown menus with hover highlighting, accelerator
  labels, icons, and separators, so that the application feels like a standard
  desktop app.

## 4. Requirements (EARS)

### Data model & persistence

- **R1 (ubiquitous):** The system shall store the menu structure for each
  MenuBar control in a YAML file named `<control-id>.menu.yaml` in the same
  directory as the `.cfrm` file.
- **R2 (ubiquitous):** The YAML file shall contain a top-level `menu` key
  holding a list of top-level menu entries, each with nested `items`.
- **R3 (ubiquitous):** Each menu item shall have the following fields:
  `id` (unique string, auto-generated), `label` (display text), `type`
  (`action` | `separator`), `icon` (optional, from built-in catalogue),
  `accelerator` (optional, e.g. `"Cmd+N"`), `action` (optional, see R5),
  `enabled` (boolean, default true), and `items` (optional, for sub-menus).
- **R4 (constraint):** The system shall support at most **3 nesting levels**:
  top bar → dropdown → submenu. Deeper nesting shall be rejected by the editor.
- **R5 (ubiquitous):** The `action` field shall support the following formats:
  - `open-form:<form-name>` — open/switch to the named form.
  - `property:<CONTROL-ID>.<PROP>=<VALUE>` — set a property on a control.
  - `close-application` — terminate the running application.
  - `event` — fire the `onMenuClick` event only (the nested COBOL event
    handler decides what to do based on the item `id`).
- **R6 (ubiquitous):** The YAML file shall include a `hash` field containing
  an HMAC-SHA256 digest of the `menu` content, keyed with a compile-time
  secret. The runtime shall validate the hash on load and refuse to render a
  menu whose hash is invalid.

### IDE tree editor

- **R7 (event):** When the user clicks "Edit Menu..." in the MenuBar's
  properties panel, the system shall open a **modal tree editor** window.
- **R8 (ubiquitous):** The tree editor shall display the menu hierarchy as an
  indented tree with expand/collapse. Each node shows: icon preview, label,
  accelerator, action summary, enabled state.
- **R9 (ubiquitous):** The tree editor shall provide buttons to: **Add item**
  (action or separator, at the selected level), **Add sub-menu**, **Delete**,
  **Move up**, **Move down**.
- **R10 (ubiquitous):** When a tree node is selected, the editor shall show a
  detail panel with editable fields: label, icon (dropdown of built-in icons),
  accelerator (text field with validation), action type (dropdown), action
  target (text field), enabled (checkbox).
- **R11 (event):** When the user clicks "Save" in the tree editor, the system
  shall write the YAML file with the computed HMAC hash and close the modal.
- **R12 (event):** When the user clicks "Cancel", the system shall discard
  changes and close the modal.

### Control properties

- **R13 (ubiquitous):** The MenuBar control shall expose the following
  properties in the properties panel:
  - `HighlightBgColor` — background colour of hovered menu items.
  - `HighlightFgColor` — foreground colour of hovered menu items.
  - `SelectedBgColor` — background colour of the active/open menu item.
  - `SelectedFgColor` — foreground colour of the active/open menu item.
- **R14 (ubiquitous):** The properties panel shall show an "Edit Menu..."
  button in the MenuBar's Basic properties section that opens the tree editor
  (R7).

### Rendering (designer canvas)

- **R15 (ubiquitous):** On the designer canvas, the MenuBar shall render the
  top-level menu labels in a horizontal bar (glass style), reading them from
  the YAML file. If no YAML file exists, show "☰ MenuBar (empty)".
- **R16 (state):** While in glass mode, the menu bar shall use the form's
  active glass style (Classic or Enhanced) for its background.

### Rendering (runtime / preview)

- **R17 (event):** When the user clicks a top-level menu label at runtime, the
  system shall open a pulldown dropdown below that label, showing the items
  defined under that menu entry.
- **R18 (ubiquitous):** Each dropdown item shall render: icon (left, 16×16 dp),
  label (centre), accelerator text (right-aligned, dimmed).
- **R19 (ubiquitous):** Separator items shall render as a thin horizontal line
  spanning the dropdown width.
- **R20 (state):** While the cursor hovers over a menu item, the system shall
  highlight it using `HighlightBgColor` / `HighlightFgColor`.
- **R21 (event):** When a menu item with sub-items is hovered, the system shall
  open the sub-menu to the right of the parent dropdown.
- **R22 (event):** When the user clicks an action item, the system shall:
  close all open menus, then execute the action (R5), then fire the
  `onMenuClick` event with the item's `id` as a parameter.
- **R23 (state):** While a menu item's `enabled` property is false, the system
  shall render it dimmed and ignore clicks on it.
- **R24 (event):** When the user presses an accelerator key combination that
  matches a menu item, the system shall execute that item's action as if it
  were clicked (R22), provided the item is enabled.
- **R25 (event):** When the user clicks outside all open menus, the system
  shall close them.

### Programmatic control

- **R26 (ubiquitous):** The system shall support enabling/disabling individual
  menu items at runtime via:
  `INVOKE <menu-id> 'SetItemEnabled' USING <item-id> <bool-value>`.
- **R27 (ubiquitous):** The system shall support checking an item's enabled
  state via:
  `SET <result> TO <menu-id>::GetItemEnabled(<item-id>)`.

### Events

- **R28 (ubiquitous):** The MenuBar control shall support the following events:
  - `onMenuClick` — fired when any action item is clicked (or its accelerator
    is pressed). The clicked item's `id` is available as a runtime parameter.
  - `onMenuOpen` — fired when a dropdown opens.
  - `onMenuClose` — fired when all dropdowns close.

### Icons

- **R29 (ubiquitous):** The system shall provide a built-in catalogue of at
  least **100 hand-drawn vector icons** covering common commercial/business
  applications, social media, email, communication, media, data, system,
  status, commerce, and file management. Categories include: Document (10+),
  Edit (12+), Navigation (10+), Action (12+), UI/View (10+), Communication
  (10+), Social (6+), People/User (6+), Media (8+), Data (8+), System (10+),
  Status (8+), Commerce (6+), File/Folder (6+).
- **R30 (ubiquitous):** Each icon shall be rendered as an egui path/stroke
  drawing (no external image files), tinted with the menu item's current
  foreground colour.

## 5. Acceptance criteria

- [ ] AC1 — A new MenuBar control shows "☰ MenuBar (empty)" when no YAML file
      exists; after opening the tree editor, adding "File > New / Save / Exit"
      and saving, the bar shows "File" on the canvas.
- [ ] AC2 — The tree editor opens as a modal, shows a tree with
      expand/collapse, and allows adding/removing/reordering items up to 3
      levels. Attempting to add a 4th level is prevented.
- [ ] AC3 — Saving the tree editor produces a valid `<id>.menu.yaml` alongside
      the `.cfrm`, containing the menu structure and a `hash` field.
- [ ] AC4 — At runtime, clicking "File" opens a dropdown with "New", "Save",
      a separator, and "Exit". Hovering highlights items. Clicking "Exit"
      executes `close-application`.
- [ ] AC5 — An item with `accelerator: "Cmd+N"` shows "⌘N" right-aligned in
      the dropdown; pressing Cmd+N triggers the item's action.
- [ ] AC6 — An item with `icon: "doc-new"` shows the vector icon to the left
      of the label.
- [ ] AC7 — `INVOKE MENU1 'SetItemEnabled' USING 'file-save' WS-FALSE` dims
      the "Save" item and prevents clicking it.
- [ ] AC8 — Setting `HighlightBgColor` to `#3366FF` changes the hover
      background in the dropdown.
- [ ] AC9 — Editing the YAML file externally (changing a label) and running
      the form shows an error/refuses to render the menu (hash mismatch).
- [ ] AC10 — ToolBar and StatusBar continue to work unchanged with their
      flat-items model.
- [ ] AC11 — All new IDE strings are in the `Tr` table with 6 translations.

## 6. Constraints & steering check

- **i18n (6 languages):** The tree editor labels, button text, property labels,
  and error messages must be added to `Tr` in all six languages. Menu item
  labels themselves are user content (not translated by the system).
- **Generated-code / regenerate contract:** The YAML file is a design artifact
  (like `.cfrm`), not generated code. No impact on the regenerate-on-action
  contract.
- **Docs (English guide):** `developers-guide-en.md` must be updated with a
  MenuBar section covering the tree editor, YAML format, actions, accelerators,
  icons, and COBOL programmatic API.
- **Fix vs feature:** This is a **fix** per the standing pre-prod override
  (menus were supposed to be functional from the beginning). Bump `z`, announce
  on f=97.

## 7. Open questions

- Q1: Should the HMAC key be derived from the project path or a fixed
  compile-time constant? (Recommendation: fixed constant — simpler, and the
  goal is tamper-evidence not encryption.)
- Q2: Should the `onMenuClick` event pass the item `id`, the item `label`, or
  both? (Recommendation: pass the `id` as WS-MENU-ITEM-ID; the developer can
  look up labels if needed.)
- Q3: Should accelerator rendering use platform-native symbols (⌘ on macOS,
  Ctrl on Windows/Linux)? (Recommendation: yes, detect at runtime.)
