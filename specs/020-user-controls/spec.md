# Spec — User Controls

- **Status:** draft
- **Folder:** specs/020-user-controls/
- **Author:** Claude (spec-driven)   **Date:** 2026-06-29

## 1. Overview

Allow the developer to create **reusable composite controls** (User Controls)
from any GroupBox and its children. A User Control is a named template that
appears in the Toolbox under a dedicated "User Controls" section. When dropped
onto a form, it creates an **independent copy** of all child controls,
preserving their relative positions and properties. The developer can then
customise each deployed instance without affecting the original or other
instances.

User Controls are stored in the project definition (`.toml`) and their layout
is persisted alongside the project. They support nesting (a User Control can
contain other User Controls), and expose all child control properties and
events via a structured API accessible from both the IDE properties panel and
COBOL at runtime.

This is analogous to Fujitsu PowerCOBOL's "User Controls" or .NET's
`UserControl` — a visual composition mechanism that lets developers build
higher-level UI blocks from primitive controls.

## 2. Goals / Non-goals

### Goals

- Create a User Control from any GroupBox by right-clicking or using a menu
  action.
- Persist User Control definitions in the project `.toml` file.
- Show User Controls in a dedicated Toolbox section.
- Deploy User Controls onto forms as independent copies.
- Allow per-instance customisation of any child control's properties.
- Preserve relative positions of child controls within the User Control.
- Support nesting (User Controls inside User Controls).
- Expose child properties via a structured API (`control.section[name/value]`)
  in both the IDE properties panel and COBOL runtime.
- Expose child events from outside (`UserCtrl1.Button1.onClick`).
- Allow removal of User Controls from the project/toolbox.
- User Control name is immutable once defined.

### Non-goals

- Inheritance or template linking (instances are independent copies).
- Saving per-instance changes back to the original User Control definition.
- Visual editor for the User Control definition (it's defined by the GroupBox
  at creation time; to change the template, delete and recreate).
- User Controls shared across projects (project-scoped only).
- User Controls as standalone distributable components.

## 3. User stories

- As a **form designer**, I want to select a GroupBox with controls and create
  a User Control from it, so that I can reuse the same layout on multiple
  forms without rebuilding it each time.
- As a **form designer**, I want to drag a User Control from the Toolbox onto
  a form, so that I get a copy of all its child controls positioned correctly.
- As a **form designer**, I want to edit individual controls inside a deployed
  User Control without affecting other instances, so that I can customise
  each placement.
- As a **COBOL developer**, I want to access the properties of controls inside
  a User Control via a structured API, so that I can read and write them at
  runtime.
- As a **COBOL developer**, I want to handle events from controls inside a
  User Control, so that I can respond to user interactions on the child
  controls.
- As a **form designer**, I want to nest User Controls inside other User
  Controls, so that I can build complex composable UI blocks.

## 4. Requirements (EARS)

### Creation

- **R1 (event):** When the user right-clicks a GroupBox on the designer canvas
  and selects "Create User Control", the system shall prompt for a name (valid
  COBOL identifier, unique within the project).
- **R2 (ubiquitous):** The User Control name shall be immutable after creation.
- **R3 (ubiquitous):** The system shall capture the GroupBox and ALL its
  descendant controls (including nested User Controls) as the User Control
  definition: their types, properties, relative positions (offsets from the
  GroupBox origin), z-order, and event bindings.
- **R4 (ubiquitous):** The User Control definition shall be stored in the
  project's `.toml` file under a `[user-controls]` section.

### Toolbox

- **R5 (ubiquitous):** The Toolbox shall display a "User Controls" section
  listing all User Controls defined in the current project.
- **R6 (event):** When the user drags a User Control from the Toolbox onto the
  designer canvas, the system shall create an independent copy of the GroupBox
  and all its descendant controls at the drop position.
- **R7 (ubiquitous):** Each deployed instance shall have a unique control ID
  (auto-generated from the User Control name + sequence number, e.g.
  `MyCard-1`, `MyCard-2`).

### Deployment (copy semantics)

- **R8 (ubiquitous):** A deployed User Control instance shall be a full,
  independent copy of all child controls. Modifying an instance shall NOT
  affect the original definition or other instances.
- **R9 (ubiquitous):** Child controls within a deployed instance shall preserve
  their relative positions from the User Control definition (offsets from the
  container's top-left corner).
- **R10 (ubiquitous):** The user shall be able to select, move, resize, and
  edit the properties of any individual child control within a deployed
  instance, just like any other control on the form.

### Nesting

- **R11 (ubiquitous):** A User Control definition may contain other User
  Controls. When deployed, the nested User Controls are also copied
  recursively as independent instances.
- **R12 (constraint):** The system shall detect and reject circular references
  (a User Control containing itself, directly or indirectly).

### Properties API

- **R13 (ubiquitous):** In the IDE properties panel, a deployed User Control
  instance shall show a grouped view of all child control properties,
  organised as `ChildControlId.Section` containing `name = value` pairs.
- **R14 (ubiquitous):** At runtime, the User Control shall expose its child
  properties via an API accessible from COBOL:
  ```
  SET WS-VALUE TO UserCtrl1::GetProperty('Button1.Caption')
  INVOKE UserCtrl1 'SetProperty'
      USING 'TextBox1.Text' 'Hello'
  ```
- **R15 (ubiquitous):** The property API shall return properties as an array
  of key-value pairs in the format `control.propertySection` containing
  `name / value` entries.

### Events

- **R16 (ubiquitous):** Each child control within a deployed User Control
  shall have its events accessible from outside, using the qualified name
  format `UserCtrl1.Button1.onClick`.
- **R17 (event):** When a child control's event fires at runtime, the system
  shall dispatch it to the form's event handler using the qualified name
  (e.g. the nested program `USERCTRL1--BUTTON1--ONCLICK`).

### Removal

- **R18 (event):** When the user deletes a User Control definition from the
  project, the system shall remove it from the Toolbox and the `.toml` file.
  Existing deployed instances on forms shall remain as regular controls
  (orphaned — they no longer reference the User Control).
- **R19 (event):** When the user deletes a deployed User Control instance
  from a form, the system shall show a confirmation dialog warning that
  all child controls AND their event handlers will be permanently removed.
  Only upon confirmation shall the system remove the container, all its
  child controls, and all associated event handler code.

### Clipboard operations (all controls)

The following operations apply to **any control** on the designer canvas, not
only User Controls. They are specified here because User Controls make them
essential (deploying a composite block is effectively a paste), but they
benefit every control type.

- **R22 (event):** When the user presses **Cmd+C** (or Edit → Copy) with one
  or more controls selected, the system shall copy the selected controls
  (and all their descendants if containers) to an internal clipboard. The
  clipboard stores the full control tree: types, properties, relative
  positions, z-order, and event bindings.
- **R23 (event):** When the user presses **Cmd+X** (or Edit → Cut) with one
  or more controls selected, the system shall copy them to the clipboard
  (as in R22) and then delete them from the form. If any control has event
  handlers, the system shall show a confirmation dialog (same as R19).
- **R24 (event):** When the user presses **Cmd+V** (or Edit → Paste), the
  system shall create new controls from the clipboard at an offset from the
  original position (+20, +20 px). Each pasted control shall receive a new
  unique ID (auto-generated). Container controls paste with all their
  children. Event bindings are copied but event handler CODE is not (the
  developer writes new handlers for the pasted controls).
- **R25 (event):** When the user presses **Cmd+D** (or Edit → Duplicate), the
  system shall perform a copy+paste in one step: clone the selected controls
  with new IDs at an offset position.
- **R26 (ubiquitous):** Multi-select shall be supported: the user can select
  multiple controls (Shift+click or rubber-band selection) and copy/cut/paste
  them as a group, preserving their relative positions.
- **R27 (event):** When the user pastes controls from one form onto a
  different form (cross-form paste), the system shall create the controls on
  the target form with new unique IDs. Event handler code is not transferred.
- **R28 (ubiquitous):** The clipboard shall persist across form switches
  within the same IDE session (copy from Form A, paste onto Form B).

### Deletion confirmation (all controls)

- **R29 (event):** When the user deletes ANY control that has event handlers
  with code, the system shall show a confirmation dialog listing the number
  of controls and event handlers that will be removed. This applies to
  single controls, containers (which remove all children), and User Control
  instances. Only upon confirmation shall the deletion proceed.
- **R30 (ubiquitous):** Deleted event handler code shall be preserved in the
  form's recycle bin (`deleted_code` in the `.cfrm`) so it can be recovered
  if the developer recreates a control with the same ID.

### Persistence

- **R20 (ubiquitous):** The User Control definition in the project `.toml`
  shall include: name, original GroupBox dimensions, and for each child
  control: type, relative position (x/y offset from container origin),
  size (w/h), z-order, and all non-default properties.
- **R21 (ubiquitous):** Deployed instances on forms shall be saved in the
  `.cfrm` file as a GroupBox with a `UserControl` property referencing the
  definition name. All child controls are saved normally (they ARE regular
  controls after deployment).

## 5. Acceptance criteria

- [ ] AC1 — Right-click a GroupBox with 3 child controls → "Create User
      Control" → enter name "CustomerCard" → appears in Toolbox under
      "User Controls".
- [ ] AC2 — Drag "CustomerCard" onto a form → a GroupBox with 3 child
      controls appears, IDs are `CustomerCard-1`, children have unique IDs.
- [ ] AC3 — Edit a TextBox inside `CustomerCard-1` (change Caption) → the
      original definition and `CustomerCard-2` (if deployed) are unaffected.
- [ ] AC4 — Create a User Control "AddressBlock" containing a User Control
      "PhoneEntry" → deploying "AddressBlock" creates both levels of
      controls correctly.
- [ ] AC5 — In the properties panel, selecting `CustomerCard-1` shows grouped
      child properties: `Button1.Caption = "Save"`, `TextBox1.Text = ""`, etc.
- [ ] AC6 — At runtime, `SET WS-VAL TO CustomerCard1::GetProperty('Button1.Caption')`
      returns `"Save"`.
- [ ] AC7 — At runtime, `Button1.onClick` inside `CustomerCard-1` fires
      the handler `CUSTOMERCARD-1--BUTTON1--ONCLICK`.
- [ ] AC8 — Delete "CustomerCard" from project → removed from Toolbox;
      existing instances on forms remain as regular GroupBoxes.
- [ ] AC9 — Attempt to create a User Control that contains itself → rejected
      with an error message.
- [ ] AC12 — Delete a deployed `CustomerCard-1` instance → confirmation dialog
      warns "This will remove 3 controls and 2 event handlers. Continue?" →
      Cancel keeps everything; Confirm removes the container, children, and
      all event handler code.
- [ ] AC10 — User Control name cannot be changed after creation.
- [ ] AC11 — All new IDE strings are in the `Tr` table with 6 translations.
- [ ] AC13 — Select a Button → Cmd+C → Cmd+V → a new Button appears at
      +20,+20 with a new unique ID. Original is unchanged.
- [ ] AC14 — Select a GroupBox with 3 children → Cmd+C → Cmd+V → a new
      GroupBox with 3 children appears, all with new IDs, relative positions
      preserved.
- [ ] AC15 — Select a Button with an onClick handler → Cmd+X → confirmation
      dialog warns about event handler removal → Confirm → Button is removed.
- [ ] AC16 — Cmd+D on a Label → duplicate appears at +20,+20 with new ID.
- [ ] AC17 — Copy a Button on Form A → switch to Form B → Cmd+V → Button
      appears on Form B with new ID.
- [ ] AC18 — Delete a Panel with 5 children (2 have event handlers) →
      confirmation dialog: "Remove 6 controls and 2 event handlers?" →
      Cancel keeps everything; Confirm removes all and saves code to
      recycle bin.

## 6. Constraints & steering check

- **i18n (6 languages):** New strings: "User Controls" toolbox section,
  "Create User Control" context menu, name prompt dialog, error messages.
  All in 6 languages.
- **Generated-code / regenerate contract:** Deployed instances are regular
  controls in the `.cfrm` — the codegen generates event handlers using
  the qualified names (`USERCTRL1--BUTTON1--ONCLICK`). No special codegen
  needed beyond the nested naming convention.
- **Docs (English guide):** Add a "User Controls" section covering creation,
  deployment, customisation, property API, and event handling.
- **Fix vs feature:** This is a **feature** (new capability). However, under
  the pre-prod override it may be treated as a fix (z bump). Confirm with
  operator.

## 7. Open questions

- **Q1:** Should the User Control definition store event handler CODE (the
  COBOL source), or only the event bindings (names)? If code is stored,
  deploying a User Control would pre-populate the event handlers.
  **Recommendation:** Store event bindings only (names). The developer writes
  handlers per-instance. Pre-populated code would be confusing when instances
  diverge.
- **Q2:** Should the `.toml` store the full control tree, or reference a
  separate `.ucfrm` file per User Control?
  **Recommendation:** Store in `.toml` for simplicity — User Controls are
  typically small (5–15 controls). A separate file adds complexity for
  little benefit.
- **Q3:** Should the Toolbox show a preview/thumbnail of the User Control?
  **Recommendation:** Show the name + icon only for v1. Thumbnails can be
  added later.
