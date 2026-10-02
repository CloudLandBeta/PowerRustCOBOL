# Spec — Agents edit a menu's designed items

- **Status:** draft
- **Folder:** specs/082-agent-menu-items/
- **Author:** Claude, for the operator   **Date:** 2026-10-02

## 1. Overview

A SideMenu's or MenuBar's designed items live in `<control id>.menu.yaml`
beside the form: a tree written by the menu editor and protected by a hash.
Since 1.80.83 Grace and the specialists can *see* that tree: the context
carries a `MENU ITEMS` block, and the System KB explains the structure. They
still cannot *change* it. A Form Designer change-set has five operations
(`deploy_control`, `set_property`, `generate_event_handler`,
`create_procedure`, `set_form_structure`), and none of them reaches a menu
file. A request as simple as "reorder Samples > Responsive Layout sub-items
A-Z" therefore ends in an explanation instead of a change.

This spec adds one change-set operation that replaces a menu's designed tree,
validated and applied through the same reviewed, undoable path as every
other operation.

## 2. Goals / Non-goals

- **Goals:**
  - One operation, `set_menu_items`, that gives a menu control a new tree:
    reorder, move between groups, regroup, rename, re-icon, change an action,
    add or remove items.
  - The IDE validates the tree before applying it and refuses it with a
    reason the agent can act on.
  - Applied as one undoable step, written through `cobolt_forms::menu::save_menu`
    so the hash stays valid.
- **Non-goals:**
  - Rows a program adds at run time (`AddItem` and the other row methods).
    They are not in the file and stay the program's.
  - The ToolBar's `ToolbarLayout`, a ListBox's `Items` and similar
    content properties. `set_property` already reaches them.
  - A new menu editor or any change to the existing one.
  - Creating a menu control. `deploy_control` does that today, with an
    empty tree.

## 3. User stories

- As a developer, I want to ask Grace to "sort the Responsive Layout items
  A-Z", so that the menu changes without my opening the menu editor.
- As a developer, I want to ask "move Charts under Samples and rename it
  Dashboards", so that restructuring a menu is one request.
- As a developer, I want to undo an agent's menu change with one Undo, the
  way I undo its other changes.

## 4. Requirements (EARS)

- **R1 (ubiquitous):** The change-set contract shall offer an operation
  `set_menu_items` with `control_id` (a SideMenu or MenuBar on the open
  form) and `items` (the whole new tree, in display order).
- **R2 (ubiquitous):** Each item in `items` shall carry `id`, `label`,
  `type` (`action` or `separator`), and optionally `icon`, `action`,
  `enabled`, `preserve_previous_form`, `badge`, `badge_style` and `items`
  (its children). These are
  the fields `cobolt_forms::menu::MenuItem` already stores.
- **R3 (event):** When an item in `items` carries the `id` of an existing
  item, the system shall keep that item's identity, so COBOL that names the
  id (a `WHEN` on `SelectedItemId`) keeps working.
- **R4 (event):** When an item has no `id`, or an id that does not follow the
  menu editor's rule (four lowercase letters, unique among the window's
  menu items and toolbar groups and buttons), the system shall give it a
  new id exactly as the menu editor would.
- **R5 (constraint):** The system shall refuse the operation, with a reason
  naming the item, when the tree nests deeper than
  `cobolt_forms::menu::MAX_DEPTH`, repeats an id, or names an action whose
  target fails `validate_menu_targets` (a missing form, or a FormFormat that
  cannot be loaded that way).
- **R6 (event):** When the new tree drops an item that exists in the
  current tree, the system shall list each dropped item in the change-set
  outcome, and shall name any handler code on the form that mentions that
  item's id, so the developer can see what still refers to it. The code
  itself shall never be changed or removed.
- **R7 (ubiquitous):** The system shall apply the operation as one undoable
  designer step: Undo restores the previous `.menu.yaml` exactly, hash
  included.
- **R8 (ubiquitous):** The system shall write the file only through
  `cobolt_forms::menu::save_menu`, never as text.
- **R9 (constraint):** The system shall not touch rows a program adds at run
  time, the menu control's properties, or any other control.
- **R10 (ubiquitous):** After the operation applies, the `MENU ITEMS` block
  of the next request's context shall show the new tree.

## 5. Acceptance criteria

- [ ] AC1 (R1, R2, R8): a change-set with `set_menu_items` for PowerDemo3's
  `SideMenu-1`, ordering *Samples > Responsive Layout* A-Z, writes a
  `.menu.yaml` that `load_menu` accepts, with those twelve items in
  alphabetical order and nothing else changed.
- [ ] AC2 (R3): every item kept by the reorder still has its old id.
- [ ] AC3 (R4): an item sent with no id gets a four-letter id unique in the
  window.
- [ ] AC4 (R5): a tree four levels deep, a repeated id, and an
  `open-form:` target that is `Standalone` are each refused with a reason
  that names the item; the file is untouched.
- [ ] AC5 (R6): dropping an item whose id appears in a handler lists the
  item and the handler in the outcome; the handler is unchanged.
- [ ] AC6 (R7): one Undo after AC1 restores the file byte for byte.
- [ ] AC7 (R10): the context built after AC1 lists the new order under
  `MENU ITEMS`.
- [ ] AC8: asked "reorder Samples > Responsive Layout sub-items in
  alphabetic order (A-Z)", Grace plans one Form Designer task whose
  change-set is a single `set_menu_items`, and the menu changes.
- [ ] AC9: `example_menus_open_their_forms` stays green.

## 6. Constraints & steering check

- **i18n (6 languages):** a refusal reason shown in the IDE goes through
  `Tr` in all six tables; the reason sent back to the agent stays English.
- **Generated code / regenerate contract:** the menu file is a sidecar the
  build already embeds (`cobolt-compiler`, `assets/menus/`); no
  generator change. A regenerated program reads the new tree.
- **System KB:** the SideMenu and MenuBar "Content structure" passages
  (`control_structure` in `cobolt-compiler`) must say that `set_menu_items`
  edits the tree, and `chunked.data` must be regenerated in the same change.
- **Agent contract:** `CHANGE_SET_CONTRACT` (`tool_exec.rs`), the lint in
  `agent_lint.rs` and the Pedantic reviewer prompts must all know the new
  operation, or a correct change-set is rejected as unknown.
- **Docs:** the Developer's Guide paragraph "What Grace sees of a menu"
  changes from "agents cannot yet rearrange those items" to how they do it.
  Guide translations are deleted per GOLDEN RULE #8.
- **Fix vs feature:** a feature. It gives agents a capability they never
  had; `features` branch, a `z` bump unless the operator raises `y`.
- **GOLDEN RULE, user code is sacred:** R6 reports code that refers to a
  dropped item and never edits it.

## 7. Open questions

- Q1: Whole-tree replacement (this draft) or smaller operations such as
  `move_menu_item` and `sort_menu_items`? Whole-tree keeps one operation and
  one validation path; smaller operations cost an agent fewer tokens on a
  large menu (PowerDemo3's has 94 items).
- Q2: Should dropping an item that a handler mentions (R6) be refused
  instead of only reported?
- Q3: Should a MenuBar's accelerators be part of R2? The field exists
  (`accelerator`) but the SideMenu does not use it.
